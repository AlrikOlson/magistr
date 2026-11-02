// Examples follow the canonical MCP responses; no native service runs here.
export function fixture() {
  const step = {
    id: 'review',
    title: 'Review changes',
    type: 'implement',
    status: 'planned',
    checks: [],
    progress: [],
    artifacts: [],
  }
  const run = {
    project_id: 'example',
    goal: {
      description: 'Finish the search feature',
      created_at: 'cycle-one',
      acceptance_criteria: ['Search finds matching pages'],
    },
    step_list: [step],
    active_step: null,
    checks: [],
    recent_progress: [],
  }
  const work = {
    id: 'search',
    title: 'Find matching pages',
    name: 'Search',
    feature: 'Search',
    status: 'to_do',
    deps: [],
    acceptance: ['Matching pages appear'],
    description: 'Help people find their pages.',
  }
  const plan = {
    project_id: 'example',
    tasks: [work],
    recent_done: [],
    next: 'search',
    omitted_active: 0,
  }
  const context = {
    version: 1,
    project_id: 'example',
    project_root: '/example',
    owner_epoch: '42',
    phase: 'planning',
    phase_cycle: 'cycle-one',
    phase_step: null,
    step: null,
    check_result: null,
    checks: [
      {
        key: 'check',
        commands: [{ name: 'check', command: 'cargo check', required: true }],
      },
    ],
  }
  return {
    step,
    run,
    work,
    plan,
    context,
    finding: {
      step_number: 3,
      purpose: 'Use the existing search service',
      outcome: 'Reuse its matching rules.',
      dependencies: [],
    },
    findings: {
      next_step_number: 4,
      pinned: [
        {
          step_number: 3,
          purpose: 'Use the existing search service',
          outcome_excerpt: 'Reuse its matching rules.',
        },
      ],
    },
    changes: {
      v: 1,
      ok: true,
      results: [
        {
          kind: 'op',
          seq: 9,
          path: 'src/search.rs',
          change: 'modify',
          diff: '+ matches(query)',
          added: 1,
          removed: 0,
        },
      ],
    },
  }
}
export function decodeRequest(tool, args) {
  if (tool !== 'execute') return { tool, args }
  const match = /^return magistr\.call\((.*)\);$/.exec(args.source)
  if (!match || args.maxOperations !== 1) throw Error('Unexpected composition')
  const [operation, parameters] = JSON.parse('[' + match[1] + ']')
  return { tool: operation, args: parameters }
}
export function compositionReply(operation, raw) {
  if (raw.deny || raw.isError) return raw
  return {
    structuredContent: {
      ok: true,
      automaticReplay: false,
      receipts: [
        {
          operation,
          result: raw.structuredContent ?? raw.result ?? raw,
          failed: false,
        },
      ],
    },
  }
}
export function answer(f, server, tool, args) {
  if (server !== 'magistr') throw Error('Unexpected server: ' + server)
  if (tool === 'execute') {
    const operation = decodeRequest(tool, args)
    return compositionReply(
      operation.tool,
      answer(f, server, operation.tool, operation.args),
    ).structuredContent
  }
  if (['workspace_status', 'read', 'undo'].includes(tool)) {
    if (tool === 'workspace_status')
      return { v: 1, ok: true, results: [{ root: '/example' }] }
    if (tool === 'read')
      return {
        v: 1,
        ok: true,
        results: [{ kind: 'lines', text: 'pub fn search() {}' }],
      }
    return f.changes
  }
  switch (tool) {
    case 'run_status':
      return f.run
    case 'plan_status':
      return f.plan
    case 'plan_focus_get': {
      const activity = {
        counts: {
          total: 1,
          idea: 0,
          to_do: 1,
          in_progress: 0,
          waiting: 0,
        },
        readiness: { ready: 1, dependent: 0, waiting: 0 },
        next_ready: 'search',
      }
      return {
        lane: args.lane,
        focus: f.focus || null,
        focus_state: f.focus
          ? { state: 'current', feature: f.focus.feature }
          : { state: 'unset' },
        workstreams: {
          current: { Search: activity },
          history: {},
          unassigned: {
            counts: {
              total: 0,
              idea: 0,
              to_do: 0,
              in_progress: 0,
              waiting: 0,
            },
            readiness: { ready: 0, dependent: 0, waiting: 0 },
            next_ready: null,
          },
          unassigned_history: { done: 0, retired: 0 },
        },
        frontier: f.focus
          ? {
              state: 'current',
              frontier: {
                feature: f.focus.feature,
                activity,
                items: [
                  {
                    id: f.work.id,
                    name: f.work.name,
                    priority: 100,
                    status: f.work.status,
                    readiness: { state: 'ready' },
                  },
                ],
              },
            }
          : { state: 'unset' },
        persistent: true,
      }
    }
    case 'context':
      return {
        ...f.context,
        step: args.step_id
          ? {
              record: { ...f.step },
              omitted_checks: 0,
              omitted_progress: 0,
              detail_limit: 6000,
            }
          : null,
      }
    case 'think_engine_status':
      return f.findings
    case 'plan_get':
      return {
        records: args.ids
          .map((id) => f.plan.tasks.find((w) => w.id === id))
          .filter(Boolean),
        unknown: [],
        returned: 1,
        fields: [],
      }
    case 'think_get_step':
      return f.finding
    default:
      throw Error('Unexpected read: ' + tool)
  }
}
