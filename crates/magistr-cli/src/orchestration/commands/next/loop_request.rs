//! The requested loop limit and roadmap track for non-graphical entry points.

#[derive(Clone, Copy, Debug)]
pub struct LoopRequest<'a> {
    pub max_iterations: u32,
    pub track: Option<&'a str>,
}
