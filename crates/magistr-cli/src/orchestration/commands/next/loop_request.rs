//! The requested loop limit and feature for non-graphical entry points.

#[derive(Clone, Copy, Debug)]
pub struct LoopRequest<'a> {
    pub max_iterations: u32,
    pub feature: Option<&'a str>,
}
