import {
  clean,
  decode,
  initial,
  sameRoot,
  status,
  summary,
  detailSummary,
  validate,
  validateAction,
} from './wire.js'
import { pane, band } from './view.js'
import { request, response } from './mcp.js'

// Disposable state. Claude unloads timers and the API environment on reload.
let model = initial()
let generation = 0
let refreshing = null
let queued = false
let poll = null
let tick = null
let stopped = false
let commandAvailable = false
let followup = null
let detailWork = null
let detailQueued = null
let actionInFlight = false
let actionGeneration = 0
const reads = new Map()
const PANE = 'magistr-work'
const READS = [
  ['ship', 'magistr', 'ship_status', {}],
  ['roadmap', 'magistr', 'roadmap_status', {}],
  ['context', 'magistr', 'context', {}],
  ['findings', 'magistr', 'think_engine_status', { verbose: true }],
  ['focus', 'magistr', 'roadmap_focus_get', {}],
  ['workspace', 'magistr', 'workspace_status', {}],
  ['changes', 'magistr', 'undo', { action: 'list', limit: 40, diff: true }],
]
function redraw($) {
  $.ui.invalidate('ui.render')
}
function fail(error) {
  return clean(error?.message || error || 'The request could not finish.', 500)
}

async function read($, server, tool, args = {}) {
  const key = server + ':' + tool
  if (reads.has(key) || reads.size >= 2)
    throw new Error(
      'A previous request is still waiting. Reconnect the service if it does not finish.',
    )
  let timer
  let timeout
  const command = request(tool, args)
  const call = $.mcp
    .call(server, command.tool, command.args)
    .then((reply) => response(tool, reply))
  reads.set(key, call)
  const work = call.finally(() => {
    if (reads.get(key) === call) reads.delete(key)
  })
  try {
    timeout = new Promise((_, reject) => {
      timer = $.clock.after(12_000, () =>
        reject(
          new Error(
            'The service did not answer in time. Reconnect it and refresh.',
          ),
        ),
      )
    })
    return await Promise.race([work, timeout])
  } finally {
    timer?.cancel()
  }
}

function scheduleRefresh($, delay = 150) {
  if (stopped || followup) return
  followup = $.clock.after(delay, () => {
    followup = null
    return refresh($)
  })
}
async function refresh($, force = false) {
  if (stopped) return
  if (refreshing) {
    if (!force) {
      queued = true
      return refreshing
    }
    const epoch = generation
    await refreshing
    if (stopped || epoch !== generation) return
    return refresh($)
  }
  const epoch = generation
  model.loading = true
  redraw($)
  const previousDetail = detailWork
  refreshing = (async () => {
    if (previousDetail) await previousDetail
    // Two calls at most. Family observations never claim an atomic snapshot.
    for (let i = 0; i < READS.length; i += 2) {
      if (stopped || epoch !== generation) return
      await Promise.all(
        READS.slice(i, i + 2).map(async ([kind, server, tool, args]) => {
          const previous = model.slots[kind]
          try {
            const options =
              kind === 'focus'
                ? { lane: 'claude-mod:' + (await $.session.id()) }
                : args
            const value = validate(kind, await read($, server, tool, options))
            const now = await $.clock.now()
            if (epoch !== generation || stopped) return
            const project = model.slots.ship?.value?.project_id
            if (
              server === 'magistr' &&
              value.project_id &&
              project &&
              project !== value.project_id
            )
              throw new Error(
                'The replies name different projects. Reconnect Magistr before acting.',
              )
            if (
              kind === 'context' &&
              previous?.value?.owner_epoch !== value.owner_epoch
            ) {
              model.detail = null
              model.detailGeneration += 1
            }
            model.slots[kind] = { value, at: now, stale: false, error: '' }
          } catch (error) {
            if (epoch !== generation || stopped) return
            model.slots[kind] = { ...previous, error: fail(error), stale: true }
          }
        }),
      )
      redraw($)
    }
    if (
      epoch === generation &&
      model.slots.context?.value?.project_id !==
        model.slots.ship?.value?.project_id
    ) {
      if (model.slots.context) {
        model.slots.context.error =
          'The replies name different projects. Reconnect Magistr before acting.'
        model.slots.context.stale = true
      }
    }
    if (
      epoch === generation &&
      model.slots.context?.value &&
      model.slots.workspace?.value &&
      !sameRoot(
        model.slots.context.value.project_root,
        model.slots.workspace.value.results[0].root,
      )
    ) {
      model.slots.workspace.error =
        'The connections reach different project folders. Review /mcp before using the views.'
      model.slots.workspace.stale = true
      if (model.slots.changes) {
        model.slots.changes.error = model.slots.workspace.error
        model.slots.changes.stale = true
      }
    }
    if (
      epoch === generation &&
      !actionInFlight &&
      ['ship', 'context', 'roadmap'].every(
        (k) =>
          model.slots[k]?.value &&
          !model.slots[k].error &&
          !model.slots[k].stale,
      )
    ) {
      if (model.uncertain)
        model.notice =
          'Records refreshed. Inspect the task or work before requesting another action.'
      model.uncertain = false
    }
  })()
  try {
    await refreshing
  } finally {
    refreshing = null
    if (epoch === generation && !stopped) {
      model.loading = false
      redraw($)
      if (model.open && model.selected)
        await select($, model.selected.kind, model.selected.item)
    }
    if (queued && !stopped && epoch === generation) {
      queued = false
      // One bounded follow-up, coalescing all observations while we read.
      scheduleRefresh($, 250)
    }
  }
}
async function select($, kind, item) {
  const serial = ++model.detailGeneration
  model.selected = { kind, item }
  model.detail = null
  model.detailStatus = 'Loading details…'
  detailQueued = { kind, item, serial, epoch: generation }
  redraw($)
  if (detailWork) return detailWork
  detailWork = drainDetails($)
  try {
    await detailWork
  } finally {
    detailWork = null
  }
}
async function drainDetails($) {
  while (detailQueued && !stopped) {
    const { kind, item, serial, epoch } = detailQueued
    detailQueued = null
    try {
      if (refreshing) await refreshing
      if (serial !== model.detailGeneration || epoch !== generation || stopped)
        continue
      const [server, tool, args, wire] =
        kind === 'task'
          ? ['magistr', 'context', { task_id: item.id }, 'context']
          : kind === 'receipt'
            ? ['magistr', 'context', { receipt_id: item }, 'context']
            : kind === 'work'
              ? ['magistr', 'roadmap_get', { ids: [item.id] }, 'work']
              : kind === 'finding'
                ? [
                    'magistr',
                    'think_get_step',
                    { step_number: item.step_number, resolve_latest: true },
                    'finding',
                  ]
                : ['magistr', 'read', { file: item, budget: 3000 }, 'source']
      const value = validate(wire, await read($, server, tool, args))
      if (serial !== model.detailGeneration || epoch !== generation || stopped)
        continue
      if (
        wire === 'context' &&
        (value.project_id !== model.slots.ship?.value?.project_id ||
          value.owner_epoch !== model.slots.context?.value?.owner_epoch)
      )
        throw new Error(
          'The connection changed. Refresh before inspecting this record.',
        )
      if (kind === 'task' && value.task?.record?.id !== item.id)
        throw new Error(
          'The reply did not name the chosen task. Refresh and try again.',
        )
      model.detail = value
      model.detailStatus =
        kind === 'receipt' && !value.receipt
          ? 'This check receipt is no longer available.'
          : ''
    } catch (error) {
      if (serial !== model.detailGeneration || epoch !== generation || stopped)
        continue
      model.detailStatus = fail(error)
    }
    redraw($)
  }
}
function switchTab($, name) {
  model.tab = name
  model.selected = null
  model.detail = null
  model.detailStatus = ''
  model.detailGeneration += 1
  model.filter = ''
  redraw($)
}
async function setBand($, enabled) {
  model.band = enabled
  try {
    await $.store.set('progress-band', enabled)
  } catch {
    model.notice =
      'The band changed for this session. Its preference could not be saved.'
  }
  redraw($)
  if (enabled) await refresh($)
}

async function action($, kind, target) {
  if (actionInFlight || model.pending || model.uncertain || stopped) return
  actionInFlight = true
  const serial = actionGeneration
  const epoch = generation
  model.pending = 'Waiting for your choice…'
  redraw($)
  let dispatched = false
  try {
    if (!model.interactive)
      throw new Error(
        'Open a terminal or Desktop session to confirm this action.',
      )
    const timeLimit = Number(model.checkTimeLimit)
    if (
      kind === 'gate' &&
      (!/^\d+$/.test(model.checkTimeLimit) ||
        !Number.isSafeInteger(timeLimit) ||
        timeLimit < 1 ||
        timeLimit > 1800)
    )
      throw new Error('Choose a time limit from 1 to 1800 seconds.')
    const label =
      kind === 'gate'
        ? 'Run check'
        : kind === 'task'
          ? 'Start task'
          : kind === 'focus'
            ? 'Choose workstream'
            : 'Mark started'
    const title =
      kind === 'gate'
        ? target.commands.map((c) => c.command).join('; ')
        : target.title || target.group
    if (kind === 'gate' && title.length > 400)
      throw new Error(
        'This command is too long to review here. Inspect and run it with your usual workflow.',
      )
    const confirmation =
      label +
      ': ' +
      clean(title, 400) +
      '?' +
      (kind === 'gate' ? '\nTime limit: ' + timeLimit + ' seconds.' : '')
    const answer = await $.ui.ask(confirmation, ['Cancel', label])
    if (
      answer !== label ||
      epoch !== generation ||
      serial !== actionGeneration ||
      stopped
    ) {
      model.notice = 'Cancelled. Nothing was requested.'
      return
    }
    // Read back immediately before admission, and never act on an old owner/work cycle.
    const oldShip = model.slots.ship?.value
    const oldOwner = model.slots.context?.value?.owner_epoch
    await refresh($, true)
    if (epoch !== generation || serial !== actionGeneration || stopped) return
    if (model.slots.context?.error || !model.slots.context?.value)
      throw new Error(
        'Progress could not be checked. Reconnect Magistr before acting.',
      )
    if (model.slots.workspace?.error || !model.slots.workspace?.value)
      throw new Error(
        'The project connection could not be checked. Review /mcp before acting.',
      )
    if (
      ['work', 'focus'].includes(kind) &&
      (model.slots.roadmap?.error || !model.slots.roadmap?.value)
    )
      throw new Error(
        'The roadmap could not be checked. Refresh before acting.',
      )
    if (model.slots.ship?.error || !model.slots.ship?.value)
      throw new Error(
        'Current work could not be checked. Refresh before trying again.',
      )
    if (
      oldShip?.objective?.created_at !==
        model.slots.ship.value.objective?.created_at ||
      oldOwner !== model.slots.context?.value?.owner_epoch
    )
      throw new Error(
        'Your work or connection changed. Review it before trying again.',
      )
    if (kind === 'task') {
      const task = model.slots.ship.value.task_list.find(
        (t) => t.id === target.id,
      )
      if (
        !task ||
        task.title !== target.title ||
        !['planned', 'blocked'].includes(task.status) ||
        model.slots.ship.value.active_task
      )
        throw new Error('This task cannot be started now. Review current work.')
    }
    if (kind === 'work' && model.slots.roadmap?.value?.next !== target.id)
      throw new Error(
        'This work is no longer the next ready item. Review the roadmap.',
      )
    if (kind === 'gate') {
      if (!model.slots.ship.value.active_task)
        throw new Error(
          'Start an explicitly chosen task before recording a check.',
        )
      if (oldShip?.active_task?.id !== model.slots.ship.value.active_task.id)
        throw new Error(
          'The active task changed. Review it before running a check.',
        )
      const choice = model.slots.context?.value?.checks?.find(
        (c) => c.key === target.key,
      )
      if (
        !choice ||
        JSON.stringify(choice.commands) !== JSON.stringify(target.commands)
      )
        throw new Error(
          'The check changed. Review its command before running it.',
        )
    }
    model.pending = label + ' requested…'
    redraw($)
    const operation =
      kind === 'gate'
        ? 'magistr_gate'
        : kind === 'task'
          ? 'start_work'
          : kind === 'focus'
            ? 'roadmap_focus_set'
            : 'roadmap_start_chunk'
    const args =
      kind === 'gate'
        ? { gate: target.key, timeout_secs: timeLimit }
        : kind === 'task'
          ? { task_id: target.id }
          : kind === 'focus'
            ? {
                lane: 'claude-mod:' + (await $.session.id()),
                group: target.group,
                mode: 'build',
              }
            : { id: target.id }
    dispatched = true
    // The normal tool path retains Claude's hooks and permission checks.
    const command = request(operation, args)
    const reply = await $.tool.call({
      tool: 'mcp__magistr__' + command.tool,
      ...command.args,
      consent:
        'The user confirmed "' +
        label +
        '" for "' +
        clean(title, 400) +
        '" in the Magistr pane.' +
        (kind === 'gate' ? ' Time limit: ' + timeLimit + ' seconds.' : ''),
    })
    if (reply?.deny || reply?.isError) {
      // Only an explicit permission refusal proves the tool was not submitted.
      if (reply?.deny) dispatched = false
      throw new Error(
        clean(reply.deny || reply.text || 'The action was refused.', 500),
      )
    }
    const result = validateAction(
      kind,
      decode(response(operation, reply)),
      target,
    )
    if (epoch !== generation || stopped) {
      model.uncertain = true
      return
    }
    if (kind === 'gate') {
      if (
        typeof result.passed !== 'boolean' ||
        typeof result.recorded !== 'boolean'
      )
        throw new Error(
          'The check returned an unsupported result. Inspect its recorded evidence.',
        )
      model.notice = result.passed
        ? result.recorded
          ? 'Check passed and was recorded.'
          : 'Check passed, but was not recorded. ' + clean(result.note, 350)
        : 'Check did not pass. Inspect its evidence.'
      if (result.gates?.some((g) => !g.passed))
        model.notice = 'Some checks did not pass. Inspect each recorded result.'
      model.detail = { gate: result }
    } else {
      model.notice =
        kind === 'task'
          ? 'Task started. Continue it with your coding workflow.'
          : kind === 'focus'
            ? 'Workstream selected. No implementation was started.'
            : 'Work marked as started. Continue it with your coding workflow.'
    }
    // Read-back after acknowledgement; the submission latch stays held throughout.
    await refresh($, true)
  } catch (error) {
    if (epoch !== generation || stopped) return
    model.notice = fail(error)
    if (dispatched) {
      model.uncertain = true
      model.notice +=
        ' The request may have taken effect. Refresh and inspect the record before trying again.'
    }
  } finally {
    actionInFlight = false
    model.pending = ''
    if (epoch !== generation && dispatched) {
      model.uncertain = true
      model.notice =
        'A request from the previous session ended. Refresh and inspect its record before acting.'
    }
    if (!stopped) redraw($)
  }
}
async function open($) {
  const surfaces = await $.session.surfaces()
  const canDraw = surfaces.some((s) => s === 'terminal' || s === 'desktop')
  if (!canDraw) {
    await refresh($)
    return { text: summary(model, 'current') }
  }
  model.open = true
  try {
    const placement = await $.ui.open({
      id: PANE,
      title: 'Magistr',
      focus: true,
      closeOnEscape: true,
      rows: 22,
    })
    if (placement?.isPlaced === false)
      throw new Error('This session cannot show a pane.')
  } catch (error) {
    model.open = false
    await refresh($)
    return {
      text:
        'The pane could not open: ' +
        fail(error) +
        '\n' +
        summary(model, 'current'),
    }
  }
  await refresh($)
  return {}
}
async function command($, e) {
  const args = e.args.trim()
  if (!args) return open($)
  if (args === 'help')
    return {
      text: 'Magistr: current, roadmap, checks, findings, changes, refresh, diagnose, band on, band off, task <title>, work <title>, finding <number>. The pane lets you inspect work and confirm actions. Existing /next and /craft skills continue coding work.',
    }
  if (args === 'refresh') {
    await refresh($)
    return { text: summary(model) }
  }
  if (args === 'band on' || args === 'band off') {
    await setBand($, args === 'band on')
    return { text: model.band ? 'Progress band on.' : 'Progress band off.' }
  }
  if (args === 'diagnose') {
    await refresh($)
    return {
      text:
        'Claude Code ' +
        model.version +
        '\n' +
        READS.map(
          ([name]) =>
            ({
              ship: 'Work',
              roadmap: 'Roadmap',
              context: 'Progress',
              findings: 'Findings',
              focus: 'Chosen workstream',
              workspace: 'Project connection',
              changes: 'Changes',
            })[name] +
            ': ' +
            status(model.slots[name]) +
            (model.slots[name]?.error ? ' · ' + model.slots[name].error : ''),
        ).join('\n') +
        '\nUse /mcp to inspect and reconnect the existing servers. Keep the existing services running. Progress details and check choices need a current Magistr build.',
    }
  }
  if (['current', 'roadmap', 'checks', 'findings', 'changes'].includes(args)) {
    switchTab($, args)
    await refresh($)
    return { text: summary(model, args) }
  }
  const match = /^(task|work|finding) (.+)$/.exec(args)
  if (match) {
    await refresh($)
    const kind = match[1],
      name = match[2]
    const list =
      kind === 'task'
        ? model.slots.ship?.value?.task_list
        : kind === 'work'
          ? model.slots.roadmap?.value?.chunks
          : []
    const item =
      kind === 'finding' &&
      /^\d+$/.test(name) &&
      Number.isSafeInteger(Number(name)) &&
      Number(name) > 0
        ? { step_number: Number(name) }
        : list?.find((t) => t.title === name)
    if (!item)
      return {
        text: 'No exact match. Inspect the list and use its full title.',
      }
    await select($, kind, item)
    return { text: model.detailStatus || detailSummary(kind, model.detail) }
  }
  return { text: 'Unknown option. Use /magistr help.' }
}
export function register(on) {
  on('session.start', async ($, e, next) => {
    stopped = false
    commandAvailable = false
    generation += 1
    model.interactive = e.isInteractive
    model.surface = e.surface
    model.version = (await $.session.version()).version
    try {
      model.band = (await $.store.get('progress-band')) === true
    } catch {
      model.band = false
    }
    model.open = (await $.ui.panes()).some((p) => p.id === PANE)
    poll?.cancel()
    tick?.cancel()
    poll = $.clock.every(30_000, () => (model.open || model.band) && refresh($))
    tick = $.clock.every(5_000, async () => {
      const now = await $.clock.now()
      let changed = false
      for (const slot of Object.values(model.slots)) {
        if (!slot.stale && now - slot.at > 45_000) {
          slot.stale = true
          changed = true
        }
      }
      if (changed) redraw($)
    })
    // Registration failures cannot break other plugins or session startup.
    try {
      await $.command.register({
        name: 'magistr',
        description: 'Inspect your work, checks, findings and changes',
        argumentHint: '[view or help]',
        immediate: true,
      })
      commandAvailable = true
    } catch (error) {
      model.notice =
        'The /magistr command is unavailable in this session. ' + fail(error)
      try {
        await $.ui.log(model.notice)
      } catch {
        /* Preserve session startup under UI restrictions. */
      }
    }
    if (model.band || model.open) scheduleRefresh($, 0)
    return next(e)
  })
  on('command.run', { command: 'magistr' }, async ($, e, next) => {
    if (!commandAvailable) return next(e)
    const interrupted = () => {
      generation += 1
      actionGeneration += 1
      model.loading = false
      model.detailGeneration += 1
      detailQueued = null
      for (const slot of Object.values(model.slots)) slot.stale = true
      model.notice = 'Interrupted. Refresh before continuing.'
      redraw($)
    }
    if (next.signal.aborted) {
      interrupted()
      return { text: 'Interrupted. Use /magistr refresh to continue.' }
    }
    next.signal.addEventListener('abort', interrupted, { once: true })
    try {
      return await command($, e)
    } finally {
      next.signal.removeEventListener('abort', interrupted)
    }
  })
  on('ui.render', { component: 'Pane' }, async ($, e, next) => {
    if (e.requestId !== PANE) return next(e)
    const elements = $.ui.resolve(e)
    return pane(elements, model, e.props.bodyColumns ?? 60, {
      tab: (name) => switchTab($, name),
      refresh: () => refresh($),
      select: (kind, item) => select($, kind, item),
      action: (kind, target) => action($, kind, target),
      timeLimit: (value) => {
        model.checkTimeLimit = clean(value, 6)
        redraw($)
      },
      findingInput: (value) => {
        model.findingInput = clean(value, 12)
        redraw($)
      },
      finding: (value) => {
        const number = Number(value)
        if (
          !/^\d+$/.test(value) ||
          !Number.isSafeInteger(number) ||
          number < 1
        ) {
          model.notice = 'Enter a finding number greater than zero.'
          redraw($)
          return
        }
        return select($, 'finding', { step_number: number })
      },
      filter: (value) => {
        model.filter = clean(value, 120)
        redraw($)
      },
      close: () => $.ui.close({ id: PANE }),
      band: () => setBand($, !model.band),
    })
  })
  on('ui.render', { component: 'AbovePrompt' }, async ($, e, next) => {
    const others = await next(e)
    if (!model.band) return others
    return band(
      $.ui.resolve(e),
      model,
      others,
      () => open($),
      e.props.bodyColumns ?? 60,
    )
  })
  on('ui.close', async ($, e, next) => {
    if (e.id === PANE) {
      model.open = false
      actionGeneration += 1
      detailQueued = null
      model.detailGeneration += 1
    }
    return next(e)
  })
  on('tool.call', { tool: /^mcp__magistr__/ }, async ($, e, next) => {
    const result = await next(e)
    // Own Mod calls use explicit refresh. Never recursively observe ourselves.
    if (next.origin.plugin !== 'magistr' && (model.open || model.band)) {
      for (const slot of Object.values(model.slots)) slot.stale = true
      scheduleRefresh($)
      redraw($)
    }
    return result
  })
  on('classic.SessionStart', async ($, e, next) => {
    if (['clear', 'resume'].includes(e.source)) {
      stopped = false
      generation += 1
      actionGeneration += 1
      model.slots = {}
      model.selected = null
      model.detail = null
      detailQueued = null
      model.detailGeneration += 1
      model.pending = actionInFlight
        ? 'A previous request is still waiting…'
        : ''
      model.uncertain = actionInFlight
      scheduleRefresh($, 0)
      redraw($)
    }
    return next(e)
  })
  on('session.end', async ($, e, next) => {
    stopped = true
    generation += 1
    poll?.cancel()
    tick?.cancel()
    queued = false
    followup?.cancel()
    followup = null
    detailQueued = null
    actionGeneration += 1
    model.detailGeneration += 1
    model.pending = ''
    return next(e)
  })
}
