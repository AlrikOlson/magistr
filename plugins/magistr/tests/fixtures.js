// Examples follow the canonical MCP responses; no native service runs here.
export function fixture() {
  const task = {
    id: 'review',
    title: 'Review changes',
    type: 'implement',
    status: 'planned',
    checks: [],
    actions: [],
    artifacts: [],
  }
  const ship = {
    project_id: 'example',
    objective: {
      description: 'Finish the search feature',
      created_at: 'cycle-one',
      acceptance_criteria: ['Search finds matching pages'],
    },
    task_list: [task],
    active_task: null,
    checks: [],
    recent_actions: [],
  }
  const work = {
    id: 'search',
    title: 'Find matching pages',
    name: 'Search',
    group: 'Search',
    status: 'pending',
    deps: [],
    acceptance: ['Matching pages appear'],
    description: 'Help people find their pages.',
  }
  const roadmap = {
    project_id: 'example',
    chunks: [work],
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
    phase_task: null,
    task: null,
    receipt: null,
    checks: [
      {
        key: 'check',
        commands: [{ name: 'check', command: 'cargo check', required: true }],
      },
    ],
  }
  return {
    task,
    ship,
    work,
    roadmap,
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
export function answer(f, server, tool, args) {
  if (server === 'workbench-mcp') {
    if (tool === 'workbench_status')
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
    case 'ship_status':
      return f.ship
    case 'roadmap_status':
      return f.roadmap
    case 'roadmap_focus_get': {
      const activity = {
        counts: { total: 1, backlog: 0, pending: 1, in_progress: 0, blocked: 0 },
        readiness: { ready: 1, waiting: 0, blocked: 0 },
        next_ready: 'search',
      }
      return {
        lane: args.lane,
        focus: f.focus || null,
        focus_state: f.focus
          ? { state: 'current', group: f.focus.group }
          : { state: 'unset' },
        workstreams: {
          current: { Search: activity },
          history: {},
          ungrouped: {
            counts: { total: 0, backlog: 0, pending: 0, in_progress: 0, blocked: 0 },
            readiness: { ready: 0, waiting: 0, blocked: 0 },
            next_ready: null,
          },
          ungrouped_history: { done: 0, obsoleted: 0 },
        },
        frontier: f.focus
          ? {
              state: 'current',
              frontier: {
                group: f.focus.group,
                activity,
                items: [{
                  id: f.work.id,
                  name: f.work.name,
                  priority: 100,
                  status: f.work.status,
                  readiness: { state: 'ready' },
                }],
              },
            }
          : { state: 'unset' },
        persistent: true,
      }
    }
    case 'magistr_context':
      return {
        ...f.context,
        task: args.task_id
          ? {
              record: { ...f.task },
              omitted_checks: 0,
              omitted_actions: 0,
              detail_limit: 6000,
            }
          : null,
      }
    case 'think_engine_status':
      return f.findings
    case 'roadmap_get':
      return {
        records: args.ids
          .map((id) => f.roadmap.chunks.find((w) => w.id === id))
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
