import { expect, mock, test } from 'claude-code/testing'
import { answer, fixture } from './fixtures.js'

// These tests require Claude's official host. Independent Node tests are separate.
const PANE = {
  plugin: 'magistr',
  component: 'Pane',
  requestId: 'magistr-work',
  viewport: { columns: 100, rows: 30 },
  props: {
    title: 'Magistr',
    isFocused: true,
    bodyColumns: 60,
    placement: 'inline',
    scroll: { offset: 0, bodyRows: 22 },
    view: {},
  },
} as const
function setup(
  on: any,
  options: { unavailable?: boolean; cancel?: boolean; refusal?: boolean } = {},
) {
  const f = fixture()
  const calls: any[] = []
  const clock = mock.clock(on)
  mock.store(on)
  on('session.start', () => ({ cwd: '/example' }))
  on('session.end', () => ({}))
  on('classic.SessionStart', () => ({}))
  on('session.version', () => ({
    value: { version: '2.1.287', base: '2.1.287' },
  }))
  on('session.surfaces', () => ({ value: ['terminal'] }))
  on('session.id', () => ({ value: 'test-session' }))
  on('command.register', () => ({ value: undefined }))
  on('ui.panes', () => ({ value: [] }))
  on('ui.open', () => ({ value: { isPlaced: true } }))
  on('ui.close', () => ({ value: undefined }))
  on('ui.log', () => ({ value: undefined }))
  on('ui.render', () => ({
    type: 'Text',
    props: {},
    children: ['Other content'],
  }))
  on('mcp.call', (_: any, e: any) =>
    options.unavailable
      ? { deny: 'The service is disconnected.' }
      : {
          value: {
            content: [],
            structuredContent: answer(f, e.server, e.tool, e.args),
          },
        },
  )
  on('tool.call', (_: any, e: any) => {
    if (e.tool === 'AskUserQuestion')
      return {
        result: {
          answers: {
            [e.questions[0].question]: options.cancel ? 'Cancel' : 'Start task',
          },
        },
      }
    calls.push(e)
    if (options.refusal) return { deny: 'Permission denied.' }
    f.task.status = 'active'
    f.ship.active_task = f.task
    return { result: { id: f.task.id, status: 'active' } }
  })
  return { f, calls, clock }
}
test('entry command and supported terminal and Desktop controls inspect evidence', async ($, on) => {
  setup(on)
  await $.session.start({
    surface: 'terminal',
    isInteractive: true,
    cwd: '/example',
  })
  await $.command.run({ command: 'magistr', args: '' })
  for (const surface of ['terminal', 'desktop'] as const) {
    const ui = await $.ui.mount({ ...PANE, surface })
    expect(
      await ui.find({ type: 'Text', text: 'Finish the search feature' }),
    ).toBeDefined()
    await ui.press({ key: 'task-review' })
    expect(
      await ui.find({
        type: 'Text',
        text: 'No checks recorded for this task.',
      }),
    ).toBeDefined()
    await ui.press({ key: 'tab-roadmap' })
    await ui.press({ key: 'work-search' })
    expect(
      await ui.find({ type: 'Text', text: '• Matching pages appear' }),
    ).toBeDefined()
    await ui.press({ key: 'tab-findings' })
    await ui.press({ key: 'step-3' })
    expect(
      await ui.find({ type: 'Text', text: 'Reuse its matching rules.' }),
    ).toBeDefined()
    await ui.press({ key: 'tab-current' })
    await ui.unmount()
  }
})
test('confirmed task start uses the normal tool path with named consent', async ($, on) => {
  const { calls } = setup(on)
  await $.session.start({
    surface: 'terminal',
    isInteractive: true,
    cwd: '/example',
  })
  await $.command.run({ command: 'magistr', args: '' })
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  await ui.press({ key: 'task-review' })
  await ui.press({ key: 'start-task' })
  expect(calls.length).toBe(1)
  expect(calls[0].tool).toBe('mcp__magistr__ship_start')
  expect(calls[0].consent).toContain('confirmed "Start task"')
  expect(
    await ui.find({
      type: 'Text',
      text: 'Task started. Continue it with your coding workflow.',
    }),
  ).toBeDefined()
  await ui.unmount()
})
test('cancelling confirmation submits no mutation', async ($, on) => {
  const { calls } = setup(on, { cancel: true })
  await $.session.start({
    surface: 'terminal',
    isInteractive: true,
    cwd: '/example',
  })
  await $.command.run({ command: 'magistr', args: '' })
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  await ui.press({ key: 'task-review' })
  await ui.press({ key: 'start-task' })
  expect(calls.length).toBe(0)
  expect(
    await ui.find({ type: 'Text', text: 'Cancelled. Nothing was requested.' }),
  ).toBeDefined()
  await ui.unmount()
})
test('host permission refusal is visible and cannot become success', async ($, on) => {
  setup(on, { refusal: true })
  await $.session.start({
    surface: 'terminal',
    isInteractive: true,
    cwd: '/example',
  })
  await $.command.run({ command: 'magistr', args: '' })
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  await ui.press({ key: 'task-review' })
  await ui.press({ key: 'start-task' })
  expect(
    await ui.find({ type: 'Text', text: /Permission denied/ }),
  ).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /^Task started/ })).toBeUndefined()
  await ui.unmount()
})
test('restricted MCP calls leave honest unavailable states', async ($, on) => {
  setup(on, { unavailable: true })
  await $.session.start({
    surface: 'terminal',
    isInteractive: true,
    cwd: '/example',
  })
  await $.command.run({ command: 'magistr', args: '' })
  const ui = await $.ui.mount({ ...PANE, surface: 'desktop' })
  expect(await ui.find({ type: 'Text', text: 'Unavailable' })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /disconnected/ })).toBeDefined()
  await ui.unmount()
})
test('observer leaves unrelated tool results unchanged', async ($, on) => {
  on('tool.call', () => ({ result: 'original' }))
  const reply = await $.tool.call({ tool: 'Bash', command: 'echo example' })
  expect(reply.result).toBe('original')
})
test('narrow controls and clear recover through explicit refresh', async ($, on) => {
  setup(on)
  await $.session.start({
    surface: 'terminal',
    isInteractive: true,
    cwd: '/example',
  })
  await $.command.run({ command: 'magistr', args: 'current' })
  await $.classic.SessionStart({ source: 'clear' })
  const ui = await $.ui.mount({
    ...PANE,
    surface: 'terminal',
    props: { ...PANE.props, bodyColumns: 36 },
  })
  await ui.press({ key: 'refresh' })
  expect(await ui.find({ key: 'tab-current' })).toBeDefined()
  expect(
    await ui.find({ type: 'Text', text: 'Finish the search feature' }),
  ).toBeDefined()
  await ui.unmount()
})
test(
  'band retains content from another plugin',
  {
    plugins: [
      {
        name: 'neighbor',
        register(on) {
          on('ui.render', { component: 'AbovePrompt' }, () => ({
            type: 'Text',
            props: {},
            children: ['Neighbor'],
          }))
        },
      },
    ],
  },
  async ($, on) => {
    setup(on)
    await $.session.start({
      surface: 'terminal',
      isInteractive: true,
      cwd: '/example',
    })
    await $.command.run({ command: 'magistr', args: 'band on' })
    const ui = await $.ui.mount({
      plugin: 'magistr',
      component: 'AbovePrompt',
      surface: 'terminal',
      viewport: { columns: 80, rows: 24 },
      props: { bodyColumns: 80 },
    })
    expect(await ui.find({ type: 'Text', text: 'Neighbor' })).toBeDefined()
    expect(await ui.find({ key: 'open-magistr' })).toBeDefined()
    await ui.unmount()
  },
)
