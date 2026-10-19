import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import {
  decode,
  validate,
  validateAction,
  phase,
  verification,
  initial,
  sameRoot,
} from '../hooks/wire.js'
import { pane, band } from '../hooks/view.js'
import { answer, fixture } from './fixtures.js'
import { elements, flatten, textOf } from './host.mjs'

test('wire decoding accepts canonical MCP and normal tool replies and rejects incompatible data', () => {
  const value = fixture().ship
  assert.throws(() =>
    validate('ship', {
      structuredContent: {
        ...value,
        checks: [{ name: 'check', passed: true, verified: 'false' }],
      },
    }),
  )
  assert.throws(() =>
    validate('context', {
      structuredContent: {
        ...fixture().context,
        task: {
          record: {
            ...fixture().task,
            checks: [{ name: 'check', passed: true, verified: 'false' }],
          },
        },
      },
    }),
  )
  assert.deepEqual(
    decode({ content: [{ type: 'text', text: JSON.stringify(value) }] }),
    value,
  )
  assert.deepEqual(
    decode({
      result: { content: [{ type: 'text', text: JSON.stringify(value) }] },
    }),
    value,
  )
  assert.deepEqual(decode({ structuredContent: value }), value)
  for (const bad of [
    null,
    [],
    { content: [{ type: 'text', text: 'bad' }] },
    { isError: true, text: 'Refused' },
    { deny: 'Denied' },
    { result: { ok: false } },
    { result: '[]' },
    { structuredContent: { text: 'x'.repeat(1048577) } },
  ])
    assert.throws(() => decode(bad))
  assert.throws(() =>
    validate('context', {
      structuredContent: { ...fixture().context, version: 2 },
    }),
  )
  assert.throws(() =>
    validate('ship', { structuredContent: { project_id: 'example' } }),
  )
  assert.throws(() =>
    validate('source', { structuredContent: { v: 2, ok: true, results: [] } }),
  )
  assert.throws(() =>
    validateAction(
      'task',
      { status: 'active', id: 'different' },
      fixture().task,
    ),
  )
})
test('focus replies preserve closed and missing choices without claiming current work', () => {
  const f = fixture()
  const base = answer(f, 'magistr', 'roadmap_focus_get', { lane: 'caller' })
  const focus = { group: 'Finished search', mode: 'build' }
  assert.doesNotThrow(() => validate('focus', { structuredContent: base }))
  for (const [state, wording] of [
    ['closed', /Finished search has no unfinished work/],
    ['missing', /Finished search is no longer on the roadmap/],
  ]) {
    const value = { ...base, focus, focus_state: { state, group: focus.group } }
    if (state === 'closed') {
      const history = { done: 1, obsoleted: 0 }
      value.workstreams = {
        ...base.workstreams,
        history: { [focus.group]: history },
      }
      value.frontier = { state, group: focus.group, history }
    } else value.frontier = { state, query: focus.group }
    const m = initial()
    m.slots.ship = { value: f.ship }
    m.slots.roadmap = { value: f.roadmap }
    m.slots.focus = { value: validate('focus', { structuredContent: value }) }
    for (const tab of ['current', 'roadmap']) {
      m.tab = tab
      const rendered = textOf(pane(elements, m, 40, {}))
      assert.match(rendered, wording)
      assert.match(rendered, /Choose another workstream/)
      assert.doesNotMatch(rendered, /Chosen workstream: Finished search/)
    }
  }
  for (const bad of [
    { ...base, focus_state: undefined },
    { ...base, focus_state: { state: 'current', group: 'Search' } },
    { ...base, focus, focus_state: { state: 'unset' } },
    { ...base, focus, focus_state: { state: 'current', group: 'Search' } },
  ])
    assert.throws(() => validate('focus', { structuredContent: bad }))
})

test('phase requires matching work and task; checks distinguish recorded evidence and optional failure', () => {
  const f = fixture()
  assert.equal(phase(f.context, f.ship), 'Planning')
  assert.equal(
    phase({ ...f.context, phase_cycle: 'other' }, f.ship),
    'Phase not reported',
  )
  assert.equal(
    phase({ ...f.context, phase_task: 'other' }, f.ship),
    'Phase not reported',
  )
  assert.equal(
    verification({ checks: [{ name: 'a', passed: true, verified: false }] }),
    'Some checks are self-reported',
  )
  assert.equal(
    verification({
      checks: [{ name: 'a', passed: false, required: false, verified: true }],
    }),
    'Checks need attention',
  )
  assert.equal(
    verification({
      checks: [
        { name: 'a', passed: false },
        { name: 'a', passed: true, verified: true },
      ],
    }),
    'Recorded checks passed',
  )
})
test('rendering is deterministic, bounded, narrow and composes an existing band', () => {
  const f = fixture(),
    m = initial()
  m.slots.ship = { value: f.ship }
  m.slots.context = { value: f.context }
  const h = {
    tab() {},
    refresh() {},
    select() {},
    action() {},
    filter() {},
    close() {},
    band() {},
  }
  const one = pane(elements, m, 40, h),
    two = pane(elements, m, 40, h)
  assert.deepEqual(JSON.stringify(one), JSON.stringify(two))
  assert.equal(one.children[0].props.flexDirection, 'column')
  assert.equal(pane(elements, m, 100, h).children[0].props.flexDirection, 'row')
  for (const e of flatten(one).filter((x) => x?.type === 'Button'))
    assert.ok(e.props.key && e.props.onPress)
  const other = elements.Text({ children: ['Another plugin'] })
  assert.equal(band(elements, m, other, () => {}, 40).children[0], other)
  f.ship.task_list = Array.from({ length: 1000 }, (_, i) => ({
    id: String(i),
    title: 'Task ' + i,
    status: 'planned',
  }))
  const many = pane(elements, m, 100, h)
  assert.equal(
    flatten(many).filter((x) => x?.props?.key?.startsWith('task-')).length,
    80,
  )
  assert.match(textOf(many), /Narrow your search/)
  assert.ok(
    !JSON.stringify(one).includes('"color"'),
    'inherits the host contrast theme',
  )
})
test('runtime effects stay in the entry module and exclude models, files, processes and permission approval', async () => {
  for (const name of ['wire.js', 'view.js']) {
    const source = await readFile(
      new URL('../hooks/' + name, import.meta.url),
      'utf8',
    )
    assert.doesNotMatch(
      source,
      /\$\.|fetch\(|process\.|node:|Date\.now|setInterval|setTimeout/,
    )
  }
  const source = await readFile(
    new URL('../hooks/register.js', import.meta.url),
    'utf8',
  )
  assert.doesNotMatch(
    source,
    /\$\.(model|process|fs|prompt|permission)|fetch\(|node:|finish_work|ship_finalize|roadmap_complete|next-loop/,
  )
  const reads = source.slice(
    source.indexOf('const READS'),
    source.indexOf('function redraw'),
  )
  assert.doesNotMatch(reads, /start_work|magistr_gate|focus_set|start_chunk/)
  assert.match(source, /consent:/)
  assert.match(source, /reads\.size >= 2/)
  assert.match(source, /actionInFlight/)
})
test('wire validation covers every rendered collection and preserves Windows and Unix folder identity', () => {
  const f = fixture()
  for (const [kind, bad] of [
    [
      'context',
      {
        ...f.context,
        task: { record: { ...f.task, actions: [{ files_touched: 'bad' }] } },
      },
    ],
    [
      'context',
      {
        ...f.context,
        receipt: { gate: 'check', target: {}, freshness: { paths: 'bad' } },
      },
    ],
    ['work', { records: [{ ...f.work, acceptance: 'bad' }] }],
    ['roadmap', { ...f.roadmap, recent_done: [null] }],
    ['finding', { ...f.finding, dependencies: [null] }],
    ['findings', { ...f.findings, pinned: [null] }],
    ['source', { v: 1, ok: true, results: [null] }],
  ])
    assert.throws(() => validate(kind, { structuredContent: bad }))
  assert.equal(
    sameRoot('D:/Code/Magistr', String.raw`\\?\d:\code\magistr`),
    true,
  )
  assert.equal(
    sameRoot('//server/share/repo', String.raw`\\?\UNC\SERVER\share\repo`),
    true,
  )
  assert.equal(sameRoot('/Repo', '/repo'), false)
  assert.equal(sameRoot('/repo', '/other'), false)
})
test('adapter documentation remains discoverable and points to existing local evidence', async () => {
  const root = new URL('../../../', import.meta.url)
  const overview = await readFile(
    new URL('docs/architecture/overview.md', root),
    'utf8',
  )
  const clients = await readFile(
    new URL('docs/architecture/clients.md', root),
    'utf8',
  )
  assert.match(overview, /claude-mod\.md/)
  assert.match(clients, /claude-mod\.md/)
  for (const file of [
    'docs/architecture/claude-mod.md',
    'docs/reference/claude-mod-verification.md',
    'plugins/magistr/README.md',
  ])
    assert.ok((await readFile(new URL(file, root), 'utf8')).length > 100)
})
