import test from 'node:test'
import assert from 'node:assert/strict'
import { answer, decodeRequest } from './fixtures.js'
import { host, deferred, textOf, flatten, elements } from './host.mjs'
const flush = async () => {
  for (let i = 0; i < 30; i++) await Promise.resolve()
}
const mutations = (h) => h.calls.filter((c) => c.mutation)

test('checks and workstream choices read back their acknowledged results', async () => {
  const h = await host()
  await h.start()
  await h.command('')
  await h.press('task-review')
  await h.press('start-task')
  assert.doesNotMatch(
    textOf(await h.render()),
    /No checks recorded for this task.*planned/,
  )
  assert.match(textOf(await h.render()), /Continue this task/)
  await h.press('tab-checks')
  await h.press('gate-check')
  assert.match(textOf(await h.render()), /Check passed and was recorded/)
  assert.equal(mutations(h).at(-1).mutation.tool, 'mcp__magistr__execute')
  assert.equal(
    decodeRequest('execute', mutations(h).at(-1).mutation).tool,
    'magistr_gate',
  )
  await h.press('tab-roadmap')
  await h.press('work-search')
  await h.press('focus-work')
  assert.equal(
    decodeRequest('execute', mutations(h).at(-1).mutation).args.lane,
    'claude-mod:example-session',
  )
  await h.press('tab-current')
  assert.match(textOf(await h.render()), /Chosen workstream: Search/)
  await h.end()
})
test('mismatched source and workflow folders refuse actions', async () => {
  const h = await host({
    read: (f, server, tool, args) => ({
      structuredContent:
        tool === 'workbench_status'
          ? { v: 1, ok: true, results: [{ root: '/different' }] }
          : answer(f, server, tool, args),
    }),
  })
  await h.start()
  await h.command('')
  await h.press('task-review')
  await h.press('start-task')
  assert.equal(mutations(h).length, 0)
  assert.match(
    textOf(await h.render()),
    /project connection could not be checked/,
  )
  await h.end()
})
test('interrupted commands fence delayed replies without cancelling the native owner', async () => {
  const wait = deferred()
  let slow = false
  const h = await host({
    read: (f, server, tool, args) =>
      slow && tool === 'ship_status'
        ? wait.promise
        : { structuredContent: answer(f, server, tool, args) },
  })
  await h.start()
  await h.command('')
  slow = true
  const controller = new AbortController()
  const pending = h.dispatch(
    'command.run',
    { command: 'magistr', args: 'refresh' },
    {},
    'engine',
    controller.signal,
  )
  await flush()
  controller.abort()
  slow = false
  wait.resolve({
    structuredContent: {
      ...h.f.ship,
      objective: { description: 'An old reply', acceptance_criteria: [] },
    },
  })
  await pending
  assert.doesNotMatch(textOf(await h.render()), /An old reply/)
  assert.match(textOf(await h.render()), /Interrupted/)
  await h.end()
})

test('entry command, all navigation, evidence and source inspection use read-only MCP', async () => {
  const h = await host()
  await h.start()
  await h.command('')
  assert.ok(h.calls.some((c) => c.open?.closeOnEscape && c.open?.focus))
  assert.match(textOf(await h.render()), /Finish the search feature/)
  await h.press('task-review')
  assert.match(textOf(await h.render()), /No checks recorded for this task/)
  await h.press('tab-roadmap')
  await h.press('work-search')
  assert.match(textOf(await h.render()), /Matching pages appear/)
  await h.press('tab-findings')
  await h.press('step-3')
  assert.match(textOf(await h.render()), /Reuse its matching rules/)
  await h.press('tab-changes')
  await h.press('read-change-9')
  assert.match(textOf(await h.render()), /pub fn search/)
  assert.equal(mutations(h).length, 0)
  assert.ok(h.maxActive() <= 2)
  await h.end()
  assert.equal(h.timers.size, 0)
})
test('text fallback works without a drawing surface or when pane admission is denied', async () => {
  for (const options of [
    { surfaces: [], interactive: false },
    { openDenied: true },
  ]) {
    const h = await host(options)
    await h.start()
    assert.match((await h.command('')).text, /Finish the search feature/)
    assert.match((await h.command('roadmap')).text, /Find matching pages/)
    assert.equal(mutations(h).length, 0)
    await h.end()
  }
})
test('explicit confirmations use the normal permission path once and never start a coding loop', async () => {
  const wait = deferred()
  const h = await host({ ask: () => wait.promise })
  await h.start()
  await h.command('')
  await h.press('task-review')
  const first = h.press('start-task')
  await flush()
  await h
    .press('start-task')
    .catch((e) => assert.match(e.message, /No control/))
  assert.equal(h.asks.length, 1)
  wait.resolve('Start task')
  await first
  assert.equal(mutations(h).length, 1)
  const request = mutations(h)[0].mutation
  assert.equal(request.tool, 'mcp__magistr__start_work')
  assert.match(request.consent, /confirmed "Start task"/)
  assert.match(textOf(await h.render()), /Task started/)
  await h.end()
})
test('cancel, closing the pane and changing the owner prevent an unadmitted request', async () => {
  for (const mode of ['cancel', 'close', 'owner']) {
    const wait = deferred()
    const h = await host({ ask: () => wait.promise })
    await h.start()
    await h.command('')
    await h.press('task-review')
    const first = h.press('start-task')
    await flush()
    if (mode === 'close') await h.dispatch('ui.close', { id: 'magistr-work' })
    if (mode === 'owner') h.f.context.owner_epoch = '43'
    wait.resolve(mode === 'cancel' ? 'Cancel' : 'Start task')
    await first
    assert.equal(mutations(h).length, 0, mode)
    await h.end()
  }
})
test('refusal and malformed mutation replies cannot be presented as success', async () => {
  for (const result of [
    { deny: 'Permission denied' },
    { isError: true, text: 'Write failed' },
    { result: { status: 'active' } },
    { result: { ok: false, message: 'Task is blocked' } },
  ]) {
    const h = await host({ mutate: () => result })
    await h.start()
    await h.command('')
    await h.press('task-review')
    await h.press('start-task')
    const text = textOf(await h.render())
    assert.doesNotMatch(text, /Task started\./)
    assert.match(text, /denied|did not confirm|blocked|failed/)
    await h.end()
  }
})
test('a request crossing clear stays latched and needs successful read-back', async () => {
  const wait = deferred()
  let failed = false
  const submitted = deferred()
  const h = await host({
    mutate: () => {
      submitted.resolve()
      return wait.promise
    },
    read: (f, server, tool, args) => {
      if (failed) throw Error('Disconnected')
      return { structuredContent: answer(f, server, tool, args) }
    },
  })
  await h.start()
  await h.command('')
  await h.press('task-review')
  const first = h.press('start-task')
  await submitted.promise
  assert.equal(mutations(h).length, 1)
  await h.dispatch('classic.SessionStart', { source: 'clear' })
  failed = true
  wait.resolve({ result: { id: 'review', status: 'active' } })
  await first
  await h.command('refresh')
  assert.match(
    textOf(await h.render()),
    /may have taken effect|previous session ended/,
  )
  assert.equal(mutations(h).length, 1)
  failed = false
  await h.command('refresh')
  assert.match(textOf(await h.render()), /Records refreshed/)
  await h.end()
})
test('stale detail replies never replace the latest selection', async () => {
  const wait = deferred()
  const h = await host({
    read: (f, server, tool, args) =>
      tool === 'context' && args.task_id
        ? wait.promise
        : { structuredContent: answer(f, server, tool, args) },
  })
  await h.start()
  await h.command('')
  const task = h.press('task-review')
  await flush()
  await h.press('tab-findings')
  const finding = h.press('step-3')
  await flush()
  wait.resolve({
    structuredContent: { ...h.f.context, task: { record: h.f.task } },
  })
  await Promise.all([task, finding])
  assert.match(textOf(await h.render()), /Reuse its matching rules/)
  assert.doesNotMatch(
    textOf(await h.render()),
    /No checks recorded for this task/,
  )
  await h.end()
})
test('timeouts retain the raw call bound, stale observations and recover after reconnect', async () => {
  const wait = deferred()
  let hang = false
  const h = await host({
    read: (f, server, tool, args) =>
      hang && tool === 'ship_status'
        ? wait.promise
        : { structuredContent: answer(f, server, tool, args) },
  })
  await h.start()
  await h.command('')
  hang = true
  const refresh = h.command('refresh')
  await flush()
  await h.advance(12000)
  await refresh
  assert.match(textOf(await h.render()), /Out of date|in time/)
  assert.ok(h.maxActive() <= 2)
  hang = false
  wait.resolve({ structuredContent: h.f.ship })
  await flush()
  await h.command('refresh')
  assert.match(textOf(await h.render()), /Current at last refresh/)
  await h.end()
})
test('tool observation preserves object identity, unrelated panes and another plugin band', async () => {
  const h = await host()
  await h.start()
  await h.command('')
  const result = { result: 'original', permission: 'untouched' }
  assert.equal(
    await h.dispatch('tool.call', { tool: 'mcp__magistr__execute' }, result),
    result,
  )
  assert.equal(await h.dispatch('tool.call', { tool: 'Bash' }, result), result)
  const other = elements.Text({ children: ['Another plugin'] })
  assert.equal(await h.render(100, 'Pane', 'other', 'terminal', other), other)
  await h.command('band on')
  assert.equal(
    (await h.render(100, 'AbovePrompt', '', 'desktop', other)).children[0],
    other,
  )
  assert.deepEqual([...h.saved.keys()], ['progress-band'])
  await h.end()
})
test('headless confirmation refusal and session end remain transparent', async () => {
  const h = await host({ interactive: false })
  await h.start()
  await h.command('')
  await h.press('task-review')
  await h.press('start-task')
  assert.equal(mutations(h).length, 0)
  assert.match(textOf(await h.render()), /confirm this action/)
  const next = { ended: true }
  assert.equal(await h.dispatch('session.end', {}, next), next)
  assert.equal(h.timers.size, 0)
})
test('observers ignore their own calls and coalesce acknowledged calls from other plugins', async () => {
  const h = await host()
  await h.start()
  await h.command('')
  const result = { result: 'unchanged' }
  const count = () => h.calls.filter((c) => c.server).length
  const before = count()
  assert.equal(
    await h.dispatch(
      'tool.call',
      { tool: 'mcp__magistr__execute' },
      result,
      'magistr',
    ),
    result,
  )
  await h.advance(200)
  assert.equal(count(), before)
  for (let i = 0; i < 8; i++)
    assert.equal(
      await h.dispatch(
        'tool.call',
        { tool: 'mcp__magistr__execute' },
        result,
        'neighbor',
      ),
      result,
    )
  await h.advance(200)
  assert.equal(count() - before, 7)
  assert.ok(h.maxActive() <= 2)
  await h.end()
})
test('confirmation cannot use a changed task, active task or configured command', async () => {
  for (const mode of ['task', 'active', 'command']) {
    const wait = deferred()
    const h = await host({ ask: () => wait.promise })
    await h.start()
    await h.command('')
    if (mode === 'command') {
      h.f.task.status = 'active'
      h.f.ship.active_task = h.f.task
      await h.command('refresh')
      await h.press('tab-checks')
    } else await h.press('task-review')
    const request = h.press(mode === 'command' ? 'gate-check' : 'start-task')
    await flush()
    if (mode === 'task') h.f.task.title = 'Changed task'
    if (mode === 'active')
      h.f.ship.active_task = { ...h.f.task, id: 'another', status: 'active' }
    if (mode === 'command')
      h.f.context.checks[0].commands[0].command = 'different check'
    wait.resolve(mode === 'command' ? 'Run check' : 'Start task')
    await request
    assert.equal(mutations(h).length, 0, mode)
    assert.match(textOf(await h.render()), /changed|cannot be started/)
    await h.end()
  }
})
test('missing, incompatible and mismatched replies leave useful honest views and refuse actions', async () => {
  for (const mode of ['missing', 'incompatible', 'project', 'detail']) {
    const h = await host({
      read: (f, server, tool, args) => {
        const v = answer(f, server, tool, args)
        if (tool === 'context') {
          if (mode === 'missing') throw Error('Not connected')
          if (mode === 'incompatible')
            return { structuredContent: { ...v, version: 2 } }
          if (mode === 'project')
            return {
              structuredContent: { ...v, project_id: 'another-project' },
            }
          if (mode === 'detail' && args.task_id)
            return {
              structuredContent: {
                ...v,
                task: {
                  ...v.task,
                  record: { ...v.task.record, id: 'another-task' },
                },
              },
            }
        }
        return { structuredContent: v }
      },
    })
    await h.start()
    await h.command('')
    await h.press('task-review')
    assert.equal(mutations(h).length, 0)
    assert.match(
      textOf(await h.render()),
      /Not connected|version|different projects|chosen task|connection changed/,
    )
    assert.equal(
      (await h.command('current')).text.includes('Finish the search feature'),
      true,
    )
    await h.end()
  }
})
test('text details describe recorded work and findings without exposing response fields', async () => {
  const h = await host({ surfaces: ['headless'] })
  await h.start()
  const task = await h.command('task Review changes')
  assert.match(task.text, /Review changes\nplanned/)
  assert.doesNotMatch(task.text, /owner_epoch|project_id|detail_limit|\{\s*"/)
  const work = await h.command('work Find matching pages')
  assert.match(work.text, /Matching pages appear/)
  const finding = await h.command('finding 3')
  assert.match(finding.text, /Reuse its matching rules/)
  await h.end()
})
test('a command collision passes the event through without hijacking another plugin', async () => {
  const h = await host({ collision: true })
  await h.start()
  const result = { text: 'Another plugin command' }
  assert.equal(
    await h.dispatch('command.run', { command: 'magistr', args: '' }, result),
    result,
  )
  assert.equal(h.calls.filter((c) => c.server).length, 0)
  await h.end()
})
test('the finding field keeps typed text and refuses invalid numbers before requesting details', async () => {
  const h = await host()
  await h.start()
  await h.command('')
  await h.press('tab-findings')
  const input = () =>
    h
      .render()
      .then((tree) =>
        flatten(tree).find((x) => x?.props?.key === 'finding-number'),
      )
  await (await input()).props.onInput('3')
  assert.equal((await input()).props.value, '3')
  const before = h.calls.filter((c) => c.tool === 'think_get_step').length
  await (await input()).props.onSubmit('0')
  assert.equal(
    h.calls.filter((c) => c.tool === 'think_get_step').length,
    before,
  )
  assert.match(textOf(await h.render()), /greater than zero/)
  await (await input()).props.onSubmit('3')
  assert.match(textOf(await h.render()), /Reuse its matching rules/)
  await h.end()
})

test('check limits are reviewed exactly and invalid limits never submit', async () => {
  const h = await host()
  await h.start()
  await h.command('')
  await h.press('task-review')
  await h.press('start-task')
  await h.press('tab-checks')
  const input = async () =>
    flatten(await h.render()).find((x) => x?.props?.key === 'check-time-limit')
  for (const value of ['0', '1801', '1.5', 'no']) {
    await (await input()).props.onInput(value)
    const calls = mutations(h).length
    const asks = h.asks.length
    await h.press('gate-check')
    assert.equal(mutations(h).length, calls)
    assert.equal(h.asks.length, asks)
    assert.match(textOf(await h.render()), /from 1 to 1800 seconds/)
  }
  await (await input()).props.onInput('900')
  await h.press('gate-check')
  assert.equal(
    decodeRequest('execute', mutations(h).at(-1).mutation).args.timeout_secs,
    900,
  )
  assert.match(mutations(h).at(-1).mutation.consent, /Time limit: 900 seconds/)
  assert.match(h.asks.at(-1).question, /Time limit: 900 seconds/)
  h.f.context.checks[0].commands[0].command = 'x'.repeat(401)
  await h.command('refresh')
  const calls = mutations(h).length
  const asks = h.asks.length
  await h.press('gate-check')
  assert.equal(mutations(h).length, calls)
  assert.equal(h.asks.length, asks)
  assert.match(textOf(await h.render()), /too long to review/)
  await h.end()
})
