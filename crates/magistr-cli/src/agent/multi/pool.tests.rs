use super::*;

#[test]
fn pool_new_is_empty() {
    let pool = AgentPool::new();
    assert_eq!(pool.total_count(), 0);
    assert_eq!(pool.active_count(), 0);
    assert!(!pool.is_all_done()); // empty pool is not "all done"
    assert!(pool.topology().is_empty());
}

#[test]
fn pool_default_same_as_new() {
    let pool = AgentPool::default();
    assert_eq!(pool.total_count(), 0);
}

#[test]
fn worker_snapshot_reflects_updates() {
    let mut pool = AgentPool::new();
    let id = AgentId::new(1);
    let role = AgentRole::Worker {
        task_id: "t1".into(),
    };
    let (cancel_tx, _cancel_rx) = mpsc::unbounded_channel();

    pool.topology.add(id, role.clone());
    pool.workers.insert(
        id,
        WorkerHandle {
            cancel_tx,
            role,
            specialized_role: None,
            status: WorkerStatus::Running,
            task_description: "test task".into(),
            scope: vec![],
            cost_usd: 0.0,
            tokens_used: 0,
        },
    );

    assert_eq!(pool.total_count(), 1);
    assert_eq!(pool.active_count(), 1);
    assert!(!pool.is_all_done());

    pool.update_worker_cost(id, 0.05);
    pool.update_worker_tokens(id, 1000);

    let snaps = pool.status_snapshot();
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].cost_usd, 0.05);
    assert_eq!(snaps[0].tokens_used, 1000);

    assert_eq!(pool.total_cost_usd(), 0.05);
    assert_eq!(pool.total_tokens_used(), 1000);
}

#[test]
fn cancel_worker_sets_terminal_status() {
    let mut pool = AgentPool::new();
    let id = AgentId::new(1);
    let role = AgentRole::Worker {
        task_id: "t1".into(),
    };
    let (cancel_tx, _cancel_rx) = mpsc::unbounded_channel();

    pool.topology.add(id, role.clone());
    pool.workers.insert(
        id,
        WorkerHandle {
            cancel_tx,
            role,
            specialized_role: None,
            status: WorkerStatus::Running,
            task_description: "test".into(),
            scope: vec![],
            cost_usd: 0.0,
            tokens_used: 0,
        },
    );

    pool.cancel_worker(id);
    assert_eq!(pool.workers[&id].status, WorkerStatus::Cancelled);
    assert!(pool.is_all_done());
    assert_eq!(pool.active_count(), 0);
}

#[test]
fn cancel_all_cancels_every_worker() {
    let mut pool = AgentPool::new();

    for i in 1..=3 {
        let id = AgentId::new(i);
        let (cancel_tx, _cancel_rx) = mpsc::unbounded_channel();
        pool.topology.add(
            id,
            AgentRole::Worker {
                task_id: format!("t{i}"),
            },
        );
        pool.workers.insert(
            id,
            WorkerHandle {
                cancel_tx,
                specialized_role: None,
                role: AgentRole::Worker {
                    task_id: format!("t{i}"),
                },
                status: WorkerStatus::Running,
                task_description: format!("task {i}"),
                scope: vec![],
                cost_usd: 0.0,
                tokens_used: 0,
            },
        );
    }

    assert_eq!(pool.active_count(), 3);
    pool.cancel_all();
    assert_eq!(pool.active_count(), 0);
    assert!(pool.is_all_done());
}

#[test]
fn cancel_already_terminal_is_noop() {
    let mut pool = AgentPool::new();
    let id = AgentId::new(1);
    let (cancel_tx, _cancel_rx) = mpsc::unbounded_channel();
    pool.topology.add(
        id,
        AgentRole::Worker {
            task_id: "t1".into(),
        },
    );
    pool.workers.insert(
        id,
        WorkerHandle {
            cancel_tx,
            specialized_role: None,
            role: AgentRole::Worker {
                task_id: "t1".into(),
            },
            status: WorkerStatus::Completed,
            task_description: "done".into(),
            scope: vec![],
            cost_usd: 0.0,
            tokens_used: 0,
        },
    );

    pool.cancel_worker(id); // Should not change status
    assert_eq!(pool.workers[&id].status, WorkerStatus::Completed);
}
