//! Stable hierarchical task ID construction.

/// Build a stable hierarchical task ID from a phase ID and task path.
///
/// Format: `"{phase_id}.{path[0]}.{path[1]}..."`.
/// Examples: `"E0.0"`, `"E0.2.1"`, `"V1.0.0.3"`.
#[must_use]
pub fn task_id(phase_id: &str, task_path: &[usize]) -> String {
    let mut id = phase_id.to_string();
    for &idx in task_path {
        id.push('.');
        id.push_str(&idx.to_string());
    }
    id
}
