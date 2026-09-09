use super::*;

#[test]
fn agent_id_display() {
    let id = AgentId::new(42);
    assert_eq!(id.to_string(), "agent-42");
    assert_eq!(id.as_u32(), 42);
}

#[test]
fn agent_role_display() {
    assert_eq!(AgentRole::Supervisor.to_string(), "supervisor");
    assert_eq!(
        AgentRole::Worker {
            task_id: "task-1".into()
        }
        .to_string(),
        "worker(task-1)"
    );
    assert_eq!(AgentRole::Merger.to_string(), "merger");
}

#[test]
fn worker_status_terminal() {
    assert!(!WorkerStatus::Idle.is_terminal());
    assert!(!WorkerStatus::Running.is_terminal());
    assert!(WorkerStatus::Completed.is_terminal());
    assert!(WorkerStatus::Failed.is_terminal());
    assert!(WorkerStatus::Cancelled.is_terminal());
}

#[test]
fn topology_add_and_query() {
    let mut topo = AgentTopology::new();
    assert!(topo.is_empty());

    let sup_id = AgentId::new(0);
    let w1_id = AgentId::new(1);
    let w2_id = AgentId::new(2);

    topo.add(sup_id, AgentRole::Supervisor);
    topo.add(
        w1_id,
        AgentRole::Worker {
            task_id: "task-a".into(),
        },
    );
    topo.add(
        w2_id,
        AgentRole::Worker {
            task_id: "task-b".into(),
        },
    );

    assert_eq!(topo.len(), 3);
    assert_eq!(topo.worker_count(), 2);

    let entry = topo.get(w1_id).expect("worker 1 should exist");
    assert_eq!(entry.id, w1_id);
    assert!(matches!(&entry.role, AgentRole::Worker { task_id } if task_id == "task-a"));

    let workers = topo.agents_with_role(&AgentRole::Worker {
        task_id: String::new(),
    });
    assert_eq!(workers.len(), 2);
}

#[test]
fn topology_remove() {
    let mut topo = AgentTopology::new();
    let id = AgentId::new(1);
    topo.add(id, AgentRole::Supervisor);
    assert_eq!(topo.len(), 1);

    topo.remove(id);
    assert!(topo.is_empty());
    assert!(topo.get(id).is_none());
}

#[test]
fn agent_msg_source_target() {
    let msg = AgentMsg::TaskAssignment {
        target: AgentId::new(1),
        prompt: "do work".into(),
        scope: vec![],
    };
    assert!(msg.source().is_none());
    assert_eq!(msg.target(), Some(AgentId::new(1)));

    let msg = AgentMsg::TaskResult {
        source: AgentId::new(2),
        success: true,
        summary: "done".into(),
        files_modified: vec![],
        cost_usd: None,
    };
    assert_eq!(msg.source(), Some(AgentId::new(2)));
    assert!(msg.target().is_none());

    let msg = AgentMsg::CancelRequest {
        target: AgentId::new(3),
        reason: "budget".into(),
    };
    assert!(msg.source().is_none());
    assert_eq!(msg.target(), Some(AgentId::new(3)));
}
