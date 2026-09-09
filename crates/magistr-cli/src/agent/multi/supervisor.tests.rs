use super::*;

#[test]
fn supervisor_config_defaults() {
    let config = SupervisorConfig::new("/tmp/project");
    assert_eq!(config.max_retries, 2);
    assert_eq!(config.cwd, PathBuf::from("/tmp/project"));
}

#[test]
fn supervisor_config_builder() {
    let budget = PoolBudget {
        max_total_cost_usd: 3.0,
        ..PoolBudget::unlimited()
    };
    let config = SupervisorConfig::new("/tmp")
        .max_retries(5)
        .budget(budget.clone());

    assert_eq!(config.max_retries, 5);
    assert_eq!(config.budget.max_total_cost_usd, 3.0);
}

#[test]
fn supervisor_new_has_empty_pool() {
    let config = SupervisorConfig::new("/tmp");
    let supervisor = Supervisor::new(config);
    assert_eq!(supervisor.pool().total_count(), 0);
    assert_eq!(supervisor.context().total_findings(), 0);
}

#[test]
fn supervisor_event_rx_taken_once() {
    let config = SupervisorConfig::new("/tmp");
    let mut supervisor = Supervisor::new(config);

    assert!(supervisor.take_event_rx().is_some());
    assert!(supervisor.take_event_rx().is_none());
}

#[test]
fn worker_result_fields() {
    let result = WorkerResult {
        agent_id: super::super::topology::AgentId::new(1),
        success: true,
        summary: "done".into(),
        cost_usd: Some(0.05),
    };
    assert!(result.success);
    assert_eq!(result.summary, "done");
    assert_eq!(result.cost_usd, Some(0.05));
}
