use super::{
    RunnerParser, runner_bun::BunRunner, runner_cargo_test::CargoTestRunner, runner_go::GoRunner,
    runner_jest::JestRunner, runner_nextest::NextestRunner, runner_pytest::PytestRunner,
    runner_vitest::VitestRunner,
};

/// The bundled first-party runner parsers in retained trust order; a
/// composition may prepend, replace or extend this list.
#[must_use]
pub fn bundled_runners() -> Vec<Box<dyn RunnerParser>> {
    vec![
        Box::new(BunRunner),
        Box::new(VitestRunner),
        Box::new(JestRunner),
        Box::new(PytestRunner),
        Box::new(NextestRunner),
        Box::new(CargoTestRunner),
        Box::new(GoRunner),
    ]
}
