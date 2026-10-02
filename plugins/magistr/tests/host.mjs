// Independent adapter contract runner. This is not Claude's test harness.
import { fixture, answer } from './fixtures.js'
let instance = 0
export const elements = Object.fromEntries(
  ['Box', 'Text', 'Button', 'Input', 'Select'].map((type) => [
    type,
    (props) => ({ type, props, children: props.children ?? [] }),
  ]),
)
export const flatten = (tree) =>
  tree == null
    ? []
    : typeof tree === 'string'
      ? [tree]
      : [tree, ...(tree.children ?? []).flatMap(flatten)]
export const textOf = (tree) =>
  flatten(tree)
    .filter((x) => typeof x === 'string')
    .join('\n')
export function deferred() {
  let resolve, reject
  const promise = new Promise((yes, no) => {
    resolve = yes
    reject = no
  })
  return { promise, resolve, reject }
}
export async function host(options = {}) {
  const hooks = []
  const f = fixture()
  const calls = [],
    asks = [],
    timers = new Set(),
    saved = new Map()
  let now = 0,
    active = 0,
    maxActive = 0
  const on = (event, filter, fn) =>
    hooks.push({
      event,
      filter: typeof filter === 'function' ? {} : filter,
      fn: fn || filter,
    })
  const timer = (delay, fn, repeat) => {
    const t = {
      at: now + delay,
      fn,
      delay,
      repeat,
      cancel: () => timers.delete(t),
    }
    timers.add(t)
    return t
  }
  const $ = {
    clock: {
      now: async () => now,
      after: (ms, fn) => timer(ms, fn, false),
      every: (ms, fn) => timer(ms, fn, true),
    },
    store: {
      get: async (key) => saved.get(key),
      set: async (key, value) => saved.set(key, value),
    },
    session: {
      version: async () => ({ version: '2.1.287' }),
      surfaces: async () => options.surfaces ?? ['terminal'],
      id: async () => 'example-session',
    },
    command: {
      register: async (e) => {
        calls.push({ command: e.name })
        if (options.collision) throw Error('Command in use')
      },
    },
    mcp: {
      call: async (server, tool, args) => {
        active++
        maxActive = Math.max(active, maxActive)
        calls.push({ server, tool, args })
        try {
          return await (options.read
            ? options.read(f, server, tool, args)
            : { structuredContent: answer(f, server, tool, args) })
        } finally {
          active--
        }
      },
    },
    tool: {
      call: async (e) => {
        calls.push({ mutation: e })
        if (options.mutate) return options.mutate(f, e)
        if (e.tool === 'mcp__magistr__ship_start') {
          f.task.status = 'active'
          f.ship.active_task = f.task
          f.context.phase_task = f.task.id
          return { result: { id: f.task.id, status: 'active' } }
        }
        if (e.tool === 'mcp__magistr__roadmap_start_chunk') {
          f.work.status = 'in_progress'
          return { result: { id: f.work.id, status: f.work.status } }
        }
        if (e.tool === 'mcp__magistr__roadmap_focus_set') {
          f.focus = { group: e.group, mode: e.mode }
          return { result: { lane: e.lane, focus: f.focus } }
        }
        return {
          result: {
            passed: true,
            recorded: true,
            verified: true,
            output: 'Finished',
            receipt: 'example-check',
          },
        }
      },
    },
    ui: {
      resolve: () => elements,
      invalidate: () => {},
      panes: async () => [],
      open: async (e) => {
        calls.push({ open: e })
        if (options.openDenied) throw Error('Panes are unavailable')
      },
      close: async (e) => dispatch('ui.close', e),
      log: (e) => calls.push({ log: e }),
      ask: async (question, choices) => {
        asks.push({ question, choices })
        return options.ask ? options.ask(question, choices) : choices[1]
      },
    },
  }
  const mod = await import('../hooks/register.js?contract=' + ++instance)
  mod.register(on)
  async function dispatch(
    event,
    e = {},
    result = {},
    origin = 'engine',
    signal = new AbortController().signal,
  ) {
    const matches = hooks.filter(
      (h) =>
        h.event === event &&
        Object.entries(h.filter).every(([key, match]) =>
          match instanceof RegExp ? match.test(e[key]) : match === e[key],
        ),
    )
    const walk = async (i) => {
      if (!matches[i]) return result
      const next = () => walk(i + 1)
      next.origin = { plugin: origin }
      next.signal = signal
      return matches[i].fn($, e, next)
    }
    return walk(0)
  }
  async function render(
    width = 100,
    component = 'Pane',
    requestId = 'magistr-work',
    surface = 'terminal',
    others = null,
  ) {
    return dispatch(
      'ui.render',
      { component, requestId, surface, props: { bodyColumns: width } },
      others,
    )
  }
  async function press(key) {
    const tree = await render()
    const button = flatten(tree).find((x) => x?.props?.key === key)
    if (!button) throw Error('No control: ' + key + '\n' + textOf(tree))
    return button.props.onPress()
  }
  async function advance(ms) {
    const target = now + ms
    for (;;) {
      const due = [...timers]
        .filter((t) => t.at <= target)
        .sort((a, b) => a.at - b.at)[0]
      if (!due) break
      now = due.at
      if (due.repeat) due.at += due.delay
      else timers.delete(due)
      await due.fn()
    }
    now = target
  }
  return {
    f,
    $,
    calls,
    asks,
    timers,
    saved,
    dispatch,
    render,
    press,
    advance,
    maxActive: () => maxActive,
    start: () =>
      dispatch('session.start', {
        surface: options.surfaces?.[0] ?? 'terminal',
        isInteractive: options.interactive !== false,
      }),
    command: (args) => dispatch('command.run', { command: 'magistr', args }),
    end: () => dispatch('session.end'),
  }
}
