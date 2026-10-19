use super::*;

#[test]
fn production_capture_preserves_the_platform_temp_store_fallback() {
    const CHILD: &str = "MAGISTR_TEST_PRODUCTION_CONFIG_FALLBACK";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "evidence::writer_config_from_env::tests::production_capture_preserves_the_platform_temp_store_fallback",
            ])
            .env_clear()
            .envs(
                // A Windows process needs its system root to start, and the
                // platform temp fallback under test is read from TEMP/TMP.
                ["SYSTEMROOT", "TEMP", "TMP"]
                    .into_iter()
                    .filter(|_| cfg!(windows))
                    .filter_map(|key| std::env::var_os(key).map(|value| (key, value))),
            )
            .env(CHILD, "1")
            .env("MAGISTR_WORKFLOW_PERSIST", "false")
            .status()
            .expect("isolated production capture");
        assert!(status.success(), "production fallback child failed");
        return;
    }
    let root = tempfile::tempdir().expect("project root");
    let config = writer_config_from_env(root.path());
    assert_eq!(
        config.data_dir(),
        std::env::temp_dir().join("magistr/workflow"),
        "production capture must retain the same fallback as earlier native writers"
    );
    assert!(!config.persistence_enabled());
    assert!(config.evidence().is_some());
    assert_eq!(config.project_root(), root.path());
}
