// Decode supplied bytes only. This module has no host or native effects.
export const LIMIT = 80
export const record = (v) =>
  v !== null && typeof v === 'object' && !Array.isArray(v)
const array = (v, key) => {
  if (!Array.isArray(v))
    throw new Error(
      'The reply did not contain ' + key + '. Update Magistr and reconnect.',
    )
  return v
}
const string = (v, key) => {
  if (typeof v !== 'string')
    throw new Error(
      'The reply did not contain ' + key + '. Update Magistr and reconnect.',
    )
  return v
}

const optionalArray = (v, key) => (v == null ? [] : array(v, key))
const taskRecord = (t) => {
  if (!record(t)) throw new Error('Task evidence could not be read.')
  string(t.id, 'a task')
  string(t.title, 'a task title')
  string(t.status, 'a task status')
  array(t.checks, 'task checks').forEach((c) => {
    string(c.name, 'a check name')
    if (typeof c.passed !== 'boolean')
      throw new Error('A check has no outcome.')
    if (c.verified != null && typeof c.verified !== 'boolean')
      throw new Error('The check evidence has an unsupported format.')
  })
  array(t.actions, 'task actions').forEach((a) => {
    optionalArray(a.files_touched, 'changed files').forEach((p) =>
      string(p, 'a file'),
    )
  })
  optionalArray(t.artifacts, 'task artifacts')
}
const workRecord = (c) => {
  if (!record(c)) throw new Error('Work details could not be read.')
  string(c.id, 'work')
  string(c.title, 'a work title')
  string(c.status, 'a work status')
  optionalArray(c.deps, 'dependencies').forEach((p) =>
    string(p, 'a dependency'),
  )
  optionalArray(c.acceptance, 'acceptance criteria').forEach((p) =>
    string(p, 'an acceptance criterion'),
  )
  optionalArray(c.cross_refs, 'linked findings').forEach((p) =>
    string(p, 'a linked finding'),
  )
}

export function clean(value, limit = 6000) {
  return String(value ?? '')
    .replace(/[\u0000-\u0008\u000b-\u001f\u007f]/g, '')
    .slice(0, limit)
}
export function decode(raw, depth = 0) {
  if (depth > 4) throw new Error('The reply has too many nested wrappers.')
  if (!record(raw)) throw new Error('No readable reply. Reconnect and refresh.')
  if (raw.deny) throw new Error(clean(raw.deny, 500))
  if (raw.isError || raw.is_error)
    throw new Error(
      clean(
        raw.text ||
          raw.content
            ?.filter((b) => b.type === 'text')
            .map((b) => b.text)
            .join('\n') ||
          'The service refused the request.',
        500,
      ),
    )
  let value = raw.structuredContent ?? raw.result ?? raw
  if (value !== raw && record(value) && Array.isArray(value.content))
    return decode(value, depth + 1)
  if (value === raw && Array.isArray(raw.content)) {
    const text = raw.content
      .filter((b) => b.type === 'text')
      .map((b) => b.text)
      .join('\n')
    if (text.length > 1_048_576)
      throw new Error('The reply is too large. Inspect a smaller selection.')
    try {
      value = JSON.parse(text)
    } catch {
      throw new Error(
        'The reply could not be read. Update Magistr and reconnect.',
      )
    }
  } else if (typeof value === 'string') {
    if (value.length > 1_048_576)
      throw new Error('The reply is too large. Inspect a smaller selection.')
    try {
      value = JSON.parse(value)
    } catch {
      throw new Error(
        'The reply could not be read. Update Magistr and reconnect.',
      )
    }
  }
  if (!record(value)) throw new Error('The reply has an unsupported format.')
  if (value.ok === false || value.error_kind || value.error) {
    throw new Error(
      clean(
        value.message ||
          value.error?.message ||
          'The service refused the request.',
        500,
      ),
    )
  }
  let serialized
  try {
    serialized = JSON.stringify(value)
  } catch {
    throw new Error('The reply could not be read.')
  }
  if (serialized.length > 1_048_576)
    throw new Error('The reply is too large. Inspect a smaller selection.')
  // Copy supplied objects so a host or reused response cannot change our observation.
  return JSON.parse(serialized)
}
export function validate(kind, raw) {
  const v = decode(raw)
  if (kind === 'ship') {
    string(v.project_id, 'the project')
    array(v.task_list, 'the tasks').forEach((t) => {
      string(t.id, 'a task')
      string(t.title, 'a task title')
      string(t.status, 'a task status')
    })
    array(v.checks, 'the checks').forEach((c) => {
      string(c.name, 'a check name')
      if (typeof c.passed !== 'boolean')
        throw new Error('A check has no outcome.')
      if (c.verified != null && typeof c.verified !== 'boolean')
        throw new Error('The check evidence has an unsupported format.')
    })
    array(v.recent_actions, 'the recorded actions')
    if (v.objective != null) {
      string(v.objective.description, 'the objective')
      array(v.objective.acceptance_criteria, 'the acceptance criteria')
    }
    if (v.active_task != null) taskRecord(v.active_task)
  } else if (kind === 'roadmap') {
    string(v.project_id, 'the project')
    array(v.chunks, 'the roadmap').forEach(workRecord)
    array(v.recent_done, 'finished work').forEach(workRecord)
  } else if (kind === 'context') {
    if (
      v.version !== 1 ||
      typeof v.owner_epoch !== 'string' ||
      !/^\d+$/.test(v.owner_epoch)
    )
      throw new Error(
        'This Magistr version cannot report progress. Update it and reconnect.',
      )
    string(v.project_id, 'the project')
    string(v.project_root, 'the project folder')
    if (
      v.phase != null &&
      ![
        'researching',
        'planning',
        'executing',
        'verifying',
        'complete',
      ].includes(v.phase)
    )
      throw new Error('The reported phase is unsupported.')
    if (v.phase_cycle != null && typeof v.phase_cycle !== 'string')
      throw new Error('The reported work is unsupported.')
    if (v.task != null) taskRecord(v.task.record)
    if (v.receipt != null) {
      if (
        !record(v.receipt) ||
        typeof v.receipt.gate !== 'string' ||
        !record(v.receipt.target)
      )
        throw new Error('The check receipt could not be read.')
      optionalArray(v.receipt.freshness?.paths, 'changed files').forEach((p) =>
        string(p, 'a file'),
      )
    }
    array(v.checks, 'available checks').forEach((c) => {
      string(c.key, 'a check choice')
      array(c.commands, 'check commands').forEach((s) => {
        string(s.name, 'a check name')
        string(s.command, 'a command')
      })
    })
  } else if (kind === 'focus') {
    string(v.lane, 'the session')
    const focusState = v.focus_state
    if (
      !record(focusState) ||
      !['unset', 'current', 'closed', 'missing'].includes(focusState.state)
    )
      throw new Error('The chosen workstream could not be checked.')
    if (v.focus != null) {
      string(v.focus.group, 'the workstream')
      string(v.focus.mode, 'the work mode')
      if (focusState.state === 'unset' || focusState.group !== v.focus.group)
        throw new Error('The chosen workstream does not match its reported state.')
    } else if (focusState.state !== 'unset') {
      throw new Error('The chosen workstream was not returned.')
    }
  } else if (kind === 'findings') {
    if (!Number.isSafeInteger(v.next_step_number))
      throw new Error('The findings reply is unsupported.')
    array(v.pinned ?? [], 'pinned findings').forEach((s) => {
      if (!record(s) || !Number.isSafeInteger(s.step_number))
        throw new Error('A recorded finding could not be read.')
      string(s.purpose, 'a finding title')
    })
  } else if (kind === 'changes' || kind === 'source' || kind === 'workspace') {
    if (v.v !== 1 || v.ok !== true)
      throw new Error(
        'This Workbench reply is unsupported. Update it and reconnect.',
      )
    array(v.results, 'the results').forEach((r) => {
      if (!record(r)) throw new Error('A Workbench result could not be read.')
    })
    if (kind === 'workspace') string(v.results[0]?.root, 'the project folder')
  } else if (kind === 'work') {
    array(v.records, 'work details').forEach(workRecord)
  } else if (kind === 'finding') {
    if (!Number.isSafeInteger(v.step_number))
      throw new Error('This finding is unavailable.')
    string(v.purpose, 'a finding title')
    optionalArray(v.dependencies, 'supporting findings').forEach((d) => {
      if (!Number.isSafeInteger(typeof d === 'number' ? d : d?.step))
        throw new Error('A supporting finding could not be read.')
    })
  }
  return v
}
export function validateAction(kind, value, target) {
  if (kind === 'gate') {
    if (
      typeof value.passed !== 'boolean' ||
      typeof value.recorded !== 'boolean'
    )
      throw new Error(
        'The check returned an unsupported result. Inspect its recorded evidence.',
      )
    const runs = value.gates ?? [value]
    if (
      !Array.isArray(runs) ||
      !runs.length ||
      runs.some(
        (g) =>
          !record(g) ||
          typeof g.passed !== 'boolean' ||
          typeof g.recorded !== 'boolean' ||
          (g.recorded && g.verified !== true),
      )
    )
      throw new Error(
        'The reply did not confirm recorded command evidence. Inspect the check before acting again.',
      )
  } else if (kind === 'focus') {
    if (value.focus?.group !== target.group || value.focus?.mode !== 'build')
      throw new Error(
        'The workstream reply did not confirm your choice. Refresh and inspect it.',
      )
  } else if (
    value.id !== target.id ||
    value.status !== (kind === 'task' ? 'active' : 'in_progress')
  ) {
    throw new Error(
      'The reply did not confirm the chosen work. Refresh and inspect it.',
    )
  }
  return value
}
export function verification(ship) {
  const checks = ship?.checks ?? []
  if (!checks.length) return 'No checks recorded'
  const latest = new Map()
  for (const c of checks) latest.set((c.task_id ?? '') + '\0' + c.name, c)
  const rows = [...latest.values()]
  if (rows.some((c) => !c.passed)) return 'Checks need attention'
  if (rows.some((c) => c.verified !== true))
    return 'Some checks are self-reported'
  return 'Recorded checks passed'
}
export function phase(context, ship) {
  if (
    context?.phase_cycle !== ship?.objective?.created_at ||
    !context?.phase ||
    context.phase_task !== (ship?.active_task?.id ?? null)
  )
    return 'Phase not reported'
  return {
    researching: 'Researching',
    planning: 'Planning',
    executing: 'Working',
    verifying: 'Checking',
    complete: 'Finished',
  }[context.phase]
}
export function titles(ship) {
  return (
    ship?.active_task?.title ||
    ship?.objective?.description ||
    'No current work'
  )
}
export function initial() {
  return {
    tab: 'current',
    selected: null,
    detail: null,
    detailStatus: '',
    band: false,
    open: false,
    interactive: false,
    surface: null,
    version: '',
    slots: {},
    loading: false,
    pending: '',
    notice: '',
    uncertain: false,
    findingInput: '',
    checkTimeLimit: '300',
    filter: '',
    detailGeneration: 0,
  }
}
export function sameRoot(left, right) {
  const normalize = (value) => {
    const path = String(value)
      .split(String.fromCharCode(92))
      .join('/')
      .replace(/^\/\/\?\/UNC\//i, '//')
      .replace(/^\/\/\?\//, '')
      .replace(/\/$/, '')
    return /^[a-z]:\//i.test(path) || path.startsWith('//')
      ? path.toLowerCase()
      : path
  }
  return (
    typeof left === 'string' &&
    typeof right === 'string' &&
    normalize(left) === normalize(right)
  )
}
export function health(m) {
  const names = { ship: 'Magistr', context: 'Progress', workspace: 'Workbench' }
  return Object.entries(names)
    .map(([key, name]) => {
      const s = m.slots[key]
      return (
        name +
        ': ' +
        (!s?.value
          ? 'unavailable'
          : s.error
            ? 'needs attention'
            : s.stale
              ? 'out of date'
              : 'connected')
      )
    })
    .join(' · ')
}
export const status = (slot) =>
  !slot
    ? 'Not loaded'
    : slot.loading
      ? 'Refreshing'
      : slot.error
        ? slot.value
          ? 'Out of date'
          : 'Unavailable'
        : slot.stale
          ? 'Out of date'
          : 'Current at last refresh'
export function summary(m, tab = m.tab) {
  const slot = m.slots[tab === 'current' || tab === 'checks' ? 'ship' : tab]
  if (!slot?.value)
    return (
      (slot?.error || 'Nothing loaded yet.') +
      '\nUse /magistr refresh to try again.'
    )
  const v = slot.value
  const lines = ['Magistr · ' + status(slot)]
  if (tab === 'current') {
    lines.push(titles(v), phase(m.slots.context?.value, v), verification(v))
    lines.push(
      ...(v.task_list ?? [])
        .slice(0, LIMIT)
        .map(
          (t) =>
            clean(t.title, 180) +
            ' · ' +
            clean(t.status.replaceAll('_', ' '), 30),
        ),
    )
  } else if (tab === 'roadmap') {
    lines.push(
      ...v.chunks
        .slice(0, LIMIT)
        .map(
          (c) =>
            clean(c.title, 180) +
            ' · ' +
            clean(c.status.replaceAll('_', ' '), 30) +
            (c.blocker_kind
              ? ' · Waiting for a blocker to clear'
              : c.id === v.next
                ? ' · Ready to start'
                : ''),
        ),
    )
    if (!v.chunks.length) lines.push('No planned work.')
    if (v.omitted_active)
      lines.push(v.omitted_active + ' more items are available in the roadmap.')
  } else if (tab === 'checks') {
    lines.push(
      verification(v),
      ...v.checks
        .slice(-LIMIT)
        .map(
          (c) =>
            clean(c.name, 140) +
            ' · ' +
            (c.passed ? 'Passed' : 'Failed') +
            (c.verified === true ? ' · Command recorded' : ' · Self-reported'),
        ),
    )
    if (!v.checks.length) lines.push('No checks recorded.')
  } else if (tab === 'findings') {
    lines.push(
      ...(v.pinned ?? [])
        .slice(0, LIMIT)
        .map(
          (s) => clean(s.purpose, 160) + ': ' + clean(s.outcome_excerpt, 240),
        ),
    )
    lines.push(
      'Next finding number: ' +
        v.next_step_number +
        '. Use /magistr finding <number> to inspect a recorded finding.',
    )
  } else if (tab === 'changes') {
    const rows = v.results
    lines.push(
      ...rows
        .slice(0, LIMIT)
        .map((r) => clean(r.file || r.path || r.kind, 180)),
    )
    if (!rows.length) lines.push('No changes recorded by Workbench.')
  }
  if (slot.error) lines.push('Refresh failed: ' + slot.error)
  return lines.join('\n')
}
export function detailSummary(kind, value) {
  if (!value) return 'No details available. Refresh and try again.'
  const lines = []
  if (kind === 'task') {
    const t = value.task?.record
    if (!t) return 'This task is no longer available.'
    lines.push(clean(t.title, 180), clean(t.status, 40).replaceAll('_', ' '))
    if (t.blocked_reason)
      lines.push('Waiting: ' + clean(t.blocked_reason, 1000))
    for (const c of t.checks.slice(-20))
      lines.push(
        clean(c.name, 140) +
          ' · ' +
          (c.passed ? 'Passed' : 'Failed') +
          (c.verified === true ? ' · Command recorded' : ' · Self-reported'),
        clean(c.command || 'No command recorded.', 1200),
        clean(c.details, 3000),
      )
    for (const a of t.actions.slice(-10))
      lines.push(
        clean(a.description, 500),
        clean(a.result, 1000),
        ...(a.files_touched ?? [])
          .slice(0, 5)
          .map((p) => 'File: ' + clean(p, 300)),
      )
    if (!t.checks.length) lines.push('No checks recorded for this task.')
  } else if (kind === 'work') {
    const w = value.records[0]
    if (!w) return 'This work is no longer available.'
    lines.push(
      clean(w.title, 180),
      clean(w.status, 40).replaceAll('_', ' '),
      clean(w.description || w.content?.summary, 2000),
    )
    lines.push(
      ...(w.acceptance ?? []).slice(0, 20).map((s) => '• ' + clean(s, 500)),
    )
    if (w.blocked_by) lines.push('Waiting: ' + clean(w.blocked_by.reason, 1000))
  } else if (kind === 'finding') {
    lines.push(
      clean(value.purpose, 180),
      clean(value.outcome, 3000),
      clean(value.context, 2000),
      clean(value.rationale, 2000),
    )
    if (value.uncertainty_notes)
      lines.push('Still uncertain: ' + clean(value.uncertainty_notes, 1000))
  }
  return clean(lines.filter(Boolean).join('\n'), 9000)
}
