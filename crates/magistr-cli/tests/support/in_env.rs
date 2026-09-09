/// Run this test in a child with explicit environment values, without mutating
/// the multithreaded test runner. The parent verifies the child and returns false.
pub fn in_env(vars: &[(&str, Option<&std::ffi::OsStr>)]) -> bool {
    const MARKER: &str = "_MAGISTR_ENV_TEST";
    let thread = std::thread::current();
    let name = thread.name().expect("a named libtest thread");
    if std::env::var_os(MARKER).as_deref() == Some(std::ffi::OsStr::new(name)) {
        return true;
    }
    let mut child = std::process::Command::new(std::env::current_exe().expect("test executable"));
    child
        .args(["--exact", name, "--nocapture"])
        .env(MARKER, name);
    for (key, value) in vars {
        match value {
            Some(value) => {
                child.env(key, value);
            }
            None => {
                child.env_remove(key);
            }
        }
    }
    let output = child.output().expect("run isolated environment test");
    assert!(
        output.status.success(),
        "isolated test {name} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("test result: ok. 1 passed;"),
        "isolated test {name} did not execute: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    false
}
