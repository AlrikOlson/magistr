import {
  clean,
  health,
  LIMIT,
  phase,
  status,
  titles,
  verification,
} from './wire.js'

const labels = {
  current: 'Work',
  roadmap: 'Roadmap',
  checks: 'Checks',
  findings: 'Findings',
  changes: 'Changes',
}
const label = (value) => clean(value, 100).replace(/_/g, ' ')
const filtered = (items, filter) =>
  items.filter((x) =>
    (x.title || x.name || x.purpose || '')
      .toLowerCase()
      .includes(filter.toLowerCase()),
  )
export function pane(E, m, width, handlers) {
  const { Box, Text, Button, Input } = E
  const text = (s, bold = false) => Text({ bold, children: [clean(s, 9000)] })
  const button = (key, name, fn, hotkey) =>
    Button({ key, label: name, onPress: fn, ...(hotkey ? { hotkey } : {}) })
  const row = (children) =>
    Box({ flexDirection: width < 48 ? 'column' : 'row', gap: 1, children })
  const ship = m.slots.ship?.value
  const slot =
    m.slots[m.tab === 'current' || m.tab === 'checks' ? 'ship' : m.tab]
  const body = []
  if (m.pending) body.push(text(m.pending, true))
  if (m.notice) body.push(text(m.notice))
  if (m.loading) body.push(text('Refreshing…'))
  body.push(text(status(slot)), text(health(m)))
  if (m.uncertain)
    body.push(
      text(
        'An earlier request may have taken effect. Refresh and inspect its record before acting.',
      ),
    )
  if (slot?.error) body.push(text('Could not refresh: ' + slot.error))
  if (m.selected) {
    const selected = m.selected
    body.push(
      button('back', 'Back to the list', () => handlers.tab(m.tab), 'b'),
    )
    body.push(
      text(
        selected.item?.title ||
          selected.item?.purpose ||
          (selected.kind === 'source' ? selected.item : 'Recorded finding'),
        true,
      ),
    )
    if (m.detailStatus) body.push(text(m.detailStatus))
    const d = m.detail
    if (d?.task) {
      const t = d.task.record
      body.push(text(label(t.status)))
      if (t.blocked_reason) body.push(text('Waiting: ' + t.blocked_reason))
      if (
        ['planned', 'blocked'].includes(t.status) &&
        !m.pending &&
        !m.uncertain
      ) {
        body.push(
          button(
            'start-task',
            'Start this task',
            () => handlers.action('task', t),
            's',
          ),
        )
      }
      if (t.status === 'active')
        body.push(text('Continue this task with your coding workflow.'))
      for (const c of (t.checks ?? []).slice(-LIMIT)) {
        body.push(
          text(
            clean(c.name, 180) +
              ' · ' +
              (c.passed ? 'Passed' : 'Failed') +
              ' · ' +
              (c.verified === true ? 'Command recorded' : 'Self-reported'),
            true,
          ),
        )
        body.push(text(c.command || 'No command was recorded.'))
        body.push(
          text(
            'Exit: ' +
              (c.exit_code ?? 'Not reported') +
              ' · ' +
              clean(c.timestamp, 100),
          ),
        )
        body.push(text(c.details || 'No further details recorded.'))
      }
      if (!t.checks?.length)
        body.push(text('No checks recorded for this task.'))
      if (d.task.omitted_checks || d.task.omitted_actions)
        body.push(
          text(
            'Earlier evidence was omitted: ' +
              d.task.omitted_checks +
              ' checks and ' +
              d.task.omitted_actions +
              ' actions.',
          ),
        )
      body.push(
        text(
          'Details show up to ' +
            d.task.detail_limit +
            ' characters per check. The files may have changed since the check. Inspect its receipt to see whether they did.',
        ),
      )
      for (const a of (t.actions ?? []).slice(-20)) {
        body.push(text(a.description, true), text(a.result))
        for (const file of (a.files_touched ?? []).slice(0, 5))
          body.push(
            button(
              'source-' + a.id + '-' + file,
              'Read ' + clean(file, 120),
              () => handlers.select('source', file),
            ),
          )
        if (a.think_step)
          body.push(
            button('reason-' + a.id, 'Inspect the recorded finding', () =>
              handlers.select('finding', { step_number: a.think_step }),
            ),
          )
      }
      for (const a of (t.artifacts ?? []).slice(0, 10))
        body.push(text(a.description + ' · ' + a.ref))
    } else if (d?.records) {
      const c = d.records[0]
      if (c) {
        body.push(
          text(label(c.status)),
          text(
            c.description ||
              c.content?.summary ||
              'No further description recorded.',
          ),
        )
        for (const a of (c.acceptance ?? []).slice(0, LIMIT))
          body.push(text('• ' + a))
        if (c.blocked_by)
          body.push(text('Waiting: ' + clean(c.blocked_by.reason, 1200)))
        for (const dep of (c.deps ?? []).slice(0, 20)) {
          const work = [
            ...(m.slots.roadmap?.value?.chunks ?? []),
            ...(m.slots.roadmap?.value?.recent_done ?? []),
          ].find((x) => x.id === dep)
          body.push(
            button(
              'dependency-' + dep,
              'Depends on: ' +
                (work
                  ? clean(work.title, 140) + ' · ' + label(work.status)
                  : 'Inspect earlier work'),
              () =>
                handlers.select(
                  'work',
                  work || { id: dep, title: 'Earlier work' },
                ),
            ),
          )
        }
        if (c.id === m.slots.roadmap?.value?.next && !m.pending && !m.uncertain)
          body.push(
            button(
              'start-work',
              'Mark this work as started',
              () => handlers.action('work', c),
              's',
            ),
          )
        if (c.group && !m.pending && !m.uncertain)
          body.push(
            button(
              'focus-work',
              'Choose this workstream',
              () =>
                handlers.action('focus', {
                  group: c.group,
                  title: c.name || c.title,
                }),
              'w',
            ),
          )
        for (const ref of (c.cross_refs ?? []).slice(0, 10)) {
          if (/^think:\d+$/.test(ref))
            body.push(
              button('ref-' + ref, 'Inspect linked finding', () =>
                handlers.select('finding', {
                  step_number: Number(ref.slice(6)),
                }),
              ),
            )
        }
      } else body.push(text('This work is no longer available.'))
    } else if (d?.step_number) {
      body.push(
        text(d.purpose, true),
        text(d.outcome),
        text(d.context),
        text(d.rationale),
        text(
          'Next: ' +
            (typeof d.next_action === 'string'
              ? d.next_action
              : d.next_action?.action || 'Not recorded'),
        ),
      )
      for (const dep of (d.dependencies ?? []).slice(0, 10)) {
        const number = typeof dep === 'number' ? dep : dep.step
        body.push(
          button(
            'finding-' + number,
            'Inspect supporting finding ' + number,
            () => handlers.select('finding', { step_number: number }),
          ),
        )
      }
      if (d.uncertainty_notes)
        body.push(text('Still uncertain: ' + d.uncertainty_notes))
    } else if (d?.receipt) {
      const receipt = d.receipt
      body.push(
        text('Check receipt', true),
        text(receipt.gate),
        text(receipt.command || 'No command recorded.'),
      )
      body.push(
        text(
          'Requested: ' +
            receipt.admitted_at +
            ' · Finished: ' +
            (receipt.settled_at || 'Still waiting'),
        ),
      )
      if (receipt.execution)
        body.push(
          text('Exit: ' + (receipt.execution.exit_code ?? 'Not reported')),
        )
      for (const file of (receipt.freshness?.paths || []).slice(0, 12))
        body.push(text(file))
      body.push(
        text(
          'Files at the time of the check: ' +
            label(receipt.freshness?.freshness || 'Unknown'),
        ),
      )
      body.push(
        text(
          'Recorded result: ' +
            label(receipt.settlement?.settlement || 'Unknown'),
        ),
      )
      if (receipt.check)
        body.push(
          text(
            receipt.check.details ||
              receipt.check.command ||
              'No additional check details.',
          ),
        )
    } else if (d?.results) {
      for (const r of d.results.slice(0, 40)) {
        if (r.text) body.push(text(r.text))
        else if (r.diff) body.push(text(r.diff))
        else if (r.kind === 'unit')
          body.push(text('File: ' + r.file + ' · ' + r.totalLines + ' lines'))
      }
      if (d.truncation)
        body.push(
          text(
            'Some source lines were omitted. Use Workbench to inspect the remaining lines.',
          ),
        )
    }
  } else if (!slot?.value) {
    body.push(
      text('Nothing loaded yet. Refresh to check the existing connections.'),
    )
  } else if (m.tab === 'current') {
    if (m.slots.focus?.value?.focus)
      body.push(
        text(
          'Chosen workstream: ' + clean(m.slots.focus.value.focus.group, 160),
        ),
      )
    body.push(
      text(titles(ship), true),
      text(phase(m.slots.context?.value, ship) + ' · ' + verification(ship)),
    )
    for (const a of (ship.objective?.acceptance_criteria ?? []).slice(0, 12))
      body.push(text('• ' + a))
    const tasks = filtered(ship.task_list ?? [], m.filter)
    body.push(
      Input({
        key: 'filter',
        label: 'Find a task',
        value: m.filter,
        onInput: (value) => handlers.filter(value),
        onSubmit: (value) => handlers.filter(value),
      }),
    )
    for (const t of tasks.slice(0, LIMIT))
      body.push(
        button(
          'task-' + t.id,
          clean(t.title, 180) + ' · ' + label(t.status),
          () => handlers.select('task', t),
        ),
      )
    if (!tasks.length)
      body.push(text(m.filter ? 'No tasks match.' : 'No tasks planned.'))
    if (tasks.length > LIMIT)
      body.push(text('More tasks are available. Narrow your search.'))
  } else if (m.tab === 'roadmap') {
    const v = slot.value
    if (m.slots.focus?.value?.focus)
      body.push(
        text(
          'Chosen workstream: ' + clean(m.slots.focus.value.focus.group, 160),
        ),
      )
    body.push(
      Input({
        key: 'filter',
        label: 'Find work',
        value: m.filter,
        onInput: (value) => handlers.filter(value),
        onSubmit: (value) => handlers.filter(value),
      }),
    )
    const items = filtered(v.chunks, m.filter)
    for (const c of items.slice(0, LIMIT))
      body.push(
        button(
          'work-' + c.id,
          clean(c.name || c.title, 140) +
            ' · ' +
            (c.blocker_kind
              ? 'Waiting for a blocker to clear'
              : c.id === v.next
                ? 'Ready to start'
                : label(c.status)),
          () => handlers.select('work', c),
        ),
      )
    if (!items.length)
      body.push(text(m.filter ? 'No work matches.' : 'No planned work.'))
    if (v.omitted_active || items.length > LIMIT)
      body.push(
        text(
          'More work is available. Narrow your search or inspect the roadmap with your usual coding workflow.',
        ),
      )
    body.push(
      text(
        'Magistr reports which work is ready. Marking work as started lets you continue it with your coding workflow.',
      ),
    )
  } else if (m.tab === 'checks') {
    body.push(text(verification(ship), true))
    for (const t of (ship.task_list ?? []).slice(0, LIMIT))
      body.push(
        button(
          'evidence-' + t.id,
          'Inspect evidence for ' + clean(t.title, 160),
          () => handlers.select('task', t),
        ),
      )
    const choices = m.slots.context?.value?.checks ?? []
    if (!m.slots.context?.value)
      body.push(
        text(
          'Available checks could not be loaded. Update Magistr and reconnect to show them.',
        ),
      )
    if (ship.active_task && !m.pending && !m.uncertain) {
      body.push(text('Choose only the checks needed for this change.', true))
      for (const c of choices.filter((c) => c.commands.length).slice(0, 30)) {
        body.push(
          button(
            'gate-' + c.key,
            {
              check: 'Check code',
              test: 'Run tests',
              lint: 'Check code style',
              structure: 'Check file structure',
              'native-write-path': 'Check native ownership',
            }[c.key] || 'Run this check',
            () => handlers.action('gate', c),
          ),
        )
        body.push(text(c.commands.map((s) => s.command).join('; ')))
      }
    } else body.push(text('Start a chosen task to record checks against it.'))
    if (m.detail?.gate) {
      for (const g of (m.detail.gate.gates || [m.detail.gate]).slice(0, 30)) {
        body.push(
          text(
            (g.passed ? 'Passed' : 'Failed') +
              ' · ' +
              (g.recorded ? 'Recorded' : 'Not recorded'),
            true,
          ),
        )
        body.push(text(g.output || g.note || 'No further output.'))
        if (g.receipt)
          body.push(
            button('receipt-' + g.receipt, 'Inspect this check receipt', () =>
              handlers.select('receipt', g.receipt),
            ),
          )
      }
    }
  } else if (m.tab === 'findings') {
    const pinned = slot.value.pinned ?? []
    body.push(text('Recorded findings', true))
    for (const s of pinned.slice(0, LIMIT))
      body.push(
        button('step-' + s.step_number, clean(s.purpose, 140), () =>
          handlers.select('finding', s),
        ),
      )
    if (!pinned.length) body.push(text('No pinned findings.'))
    const latest = slot.value.next_step_number - 1
    if (latest > 0)
      body.push(
        button(
          'latest-finding',
          'Inspect the latest finding',
          () => handlers.select('finding', { step_number: latest }),
          'l',
        ),
      )
    body.push(
      Input({
        key: 'finding-number',
        label: 'Finding number',
        value: m.findingInput,
        onInput: (value) => handlers.findingInput(value),
        onSubmit: (value) => handlers.finding(value),
      }),
    )
  } else {
    const rows = slot.value.results.filter((r) => r.kind !== 'cursor')
    for (const r of rows.slice(0, 40)) {
      body.push(text(r.file || r.path || 'Recorded change', true))
      body.push(
        text(
          r.diff ||
            r.description ||
            label(r.change || r.kind) +
              ' · ' +
              (r.added ?? 0) +
              ' lines added · ' +
              (r.removed ?? 0) +
              ' removed',
        ),
      )
      if (r.file || r.path)
        body.push(
          button('read-change-' + (r.seq ?? r.file), 'Read this file', () =>
            handlers.select('source', r.file || r.path),
          ),
        )
    }
    if (!rows.length)
      body.push(
        text('No changes recorded by Workbench. Other file changes may exist.'),
      )
    if (slot.value.truncation)
      body.push(
        text(
          'Some changes were omitted. Inspect the Workbench journal for more.',
        ),
      )
  }
  return Box({
    flexDirection: 'column',
    gap: 1,
    children: [
      row(
        Object.entries(labels).map(([name, title], i) =>
          Button({
            key: 'tab-' + name,
            label: title,
            hotkey: String(i + 1),
            plain: true,
            ...(name === m.tab ? { autoFocus: true } : {}),
            onPress: () => handlers.tab(name),
          }),
        ),
      ),
      ...body,
      row([
        button('refresh', 'Refresh', handlers.refresh, 'r'),
        button(
          'band',
          m.band ? 'Hide progress band' : 'Show progress band',
          handlers.band,
          'p',
        ),
        button('close', 'Close', handlers.close, 'q'),
      ]),
      text('Tab moves between controls · Enter chooses · Esc closes'),
    ],
  })
}
export function band(E, m, others, open, width) {
  const { Box, Text, Button } = E
  const ship = m.slots.ship
  const words =
    width < 55
      ? status(ship) + ' · ' + phase(m.slots.context?.value, ship?.value)
      : clean(titles(ship?.value), 65) +
        ' · ' +
        phase(m.slots.context?.value, ship?.value) +
        ' · ' +
        verification(ship?.value) +
        ' · ' +
        status(ship)
  return Box({
    flexDirection: 'column',
    children: [
      ...(others == null ? [] : [others]),
      Box({
        flexDirection: 'row',
        gap: 1,
        children: [
          Button({ key: 'open-magistr', label: 'Magistr', onPress: open }),
          Text({ children: [words] }),
        ],
      }),
      Text({ children: [health(m)] }),
    ],
  })
}
