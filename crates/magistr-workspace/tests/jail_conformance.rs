use magistr_workspace::jail::JailPolicy;
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    candidate: String,
    write: bool,
    code: Option<String>,
    relative: Option<String>,
}

#[test]
fn jail_enforces_current_protected_paths() {
    let fixtures: Vec<Fixture> =
        serde_json::from_str(include_str!("../fixtures/contracts/jail.json")).unwrap();
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let policy = JailPolicy::new(root.path()).unwrap();
    for fixture in fixtures {
        let candidate = fixture
            .candidate
            .replace("$root", &root.path().to_string_lossy())
            .replace("$outside", &outside.path().to_string_lossy());
        let decision = if fixture.write {
            policy.check_write(&candidate)
        } else {
            policy.check_read(&candidate)
        };
        if let Some(code) = fixture.code {
            assert_eq!(
                decision.expect_err(&fixture.candidate).code(),
                code,
                "{}",
                fixture.candidate
            );
        } else {
            let admitted = decision.expect(&fixture.candidate);
            let relative = admitted
                .strip_prefix(policy.root())
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            assert_eq!(Some(relative), fixture.relative, "{}", fixture.candidate);
        }
    }
}
