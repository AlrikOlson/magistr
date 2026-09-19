use super::*;

#[test]
fn catalog_has_at_least_21_entries() {
    assert!(
        CATALOG.len() >= 21,
        "expected at least 21 servers, got {}",
        CATALOG.len()
    );
}

#[test]
fn recommend_typescript_includes_playwright() {
    let recs = recommend("typescript", "next");
    let keys: Vec<&str> = recs.iter().map(|r| r.key).collect();
    assert!(keys.contains(&"playwright"));
    assert!(keys.contains(&"context7"));
}

#[test]
fn recommend_typescript_includes_new_servers() {
    let recs = recommend("typescript", "next");
    let keys: Vec<&str> = recs.iter().map(|r| r.key).collect();
    assert!(keys.contains(&"cypress"));
    assert!(keys.contains(&"eslint"));
    assert!(keys.contains(&"vercel"));
}

#[test]
fn recommend_rust_excludes_playwright() {
    let recs = recommend("rust", "none");
    let keys: Vec<&str> = recs.iter().map(|r| r.key).collect();
    assert!(!keys.contains(&"playwright"));
    // Universal servers still included
    assert!(keys.contains(&"context7"));
    assert!(keys.contains(&"memory"));
}

#[test]
fn recommend_includes_framework_specific() {
    let recs = recommend("python", "django");
    let keys: Vec<&str> = recs.iter().map(|r| r.key).collect();
    assert!(keys.contains(&"postgres"));
    assert!(keys.contains(&"redis"));
    assert!(keys.contains(&"mysql"));
}

#[test]
fn find_by_key() {
    assert!(find("context7").is_some());
    assert!(find("redis").is_some());
    assert!(find("docker").is_some());
    assert!(find("nonexistent").is_none());
}

#[test]
fn find_new_database_servers() {
    assert!(find("redis").is_some());
    assert!(find("mongodb").is_some());
    assert!(find("mysql").is_some());
}

#[test]
fn find_new_cloud_servers() {
    assert!(find("aws").is_some());
    assert!(find("cloudflare").is_some());
}

#[test]
fn find_new_devops_servers() {
    assert!(find("linear").is_some());
    assert!(find("docker").is_some());
    assert!(find("vercel").is_some());
}

#[test]
fn to_mcp_value_structure() {
    let entry = find("context7").unwrap();
    let val = entry.to_mcp_value();
    assert_eq!(val["command"], "npx");
    assert_eq!(val["type"], "stdio");
    assert!(val["args"].is_array());
}

#[test]
fn all_entries_have_required_fields() {
    for entry in CATALOG {
        assert!(!entry.key.is_empty(), "key is empty");
        assert!(!entry.name.is_empty(), "name is empty for {}", entry.key);
        assert!(
            !entry.description.is_empty(),
            "description is empty for {}",
            entry.key
        );
        assert!(
            !entry.command.is_empty(),
            "command is empty for {}",
            entry.key
        );
        assert!(!entry.args.is_empty(), "args is empty for {}", entry.key);
        assert!(
            !entry.categories.is_empty(),
            "categories is empty for {}",
            entry.key
        );
        assert!(
            !entry.version.is_empty(),
            "version is empty for {}",
            entry.key
        );
    }
}

#[test]
fn all_keys_are_unique() {
    let mut seen = std::collections::HashSet::new();
    for entry in CATALOG {
        assert!(seen.insert(entry.key), "duplicate key: {}", entry.key);
    }
}

#[test]
fn specific_results_come_first() {
    let recs = recommend("typescript", "next");
    // First result should be a specific match (has non-empty languages/frameworks)
    if recs.len() >= 2 {
        let first = recs[0];
        assert!(
            !first.languages.is_empty() || !first.frameworks.is_empty(),
            "specific results should come before universal ones"
        );
    }
}

// ── Install verification tests ─────────────────────────────────────

#[test]
fn install_check_npx_always_available() {
    assert_eq!(
        InstallCheck::Npx.check_with(|_| false, |_| false),
        InstallStatus::Available
    );
}

#[test]
fn install_check_binary_missing() {
    let status = InstallCheck::Binary("fixture-tool").check_with(|_| false, |_| true);
    assert_eq!(status, InstallStatus::MissingBinary("fixture-tool"));
    assert!(!status.is_available());
}

#[test]
fn install_check_env_var_missing() {
    let status = InstallCheck::NpxWithEnv("FIXTURE_KEY").check_with(|_| true, |_| false);
    assert_eq!(status, InstallStatus::MissingEnvVar("FIXTURE_KEY"));
    assert!(!status.is_available());
}

#[test]
fn install_check_env_var_present() {
    let status = InstallCheck::NpxWithEnv("FIXTURE_KEY").check_with(|_| false, |_| true);
    assert_eq!(status, InstallStatus::Available);
    assert!(status.is_available());
}

#[test]
fn install_check_binary_with_env_missing_binary() {
    let status =
        InstallCheck::BinaryWithEnv("fixture-tool", "FIXTURE_KEY").check_with(|_| false, |_| true);
    assert_eq!(status, InstallStatus::MissingBinary("fixture-tool"));
}

#[test]
fn check_available_delegates_to_install_check() {
    let entry = find("context7").unwrap();
    // Npx servers are always available, whatever the environment says.
    assert!(entry.check_available(|_| false).is_available());
    // An env-gated server answers from the injected probe, never the
    // process environment.
    let postgres = find("postgres").unwrap();
    assert_eq!(
        postgres.check_available(|_| false),
        InstallStatus::MissingEnvVar("POSTGRES_URL")
    );
    assert!(
        postgres
            .check_available(|var| var == "POSTGRES_URL")
            .is_available()
    );
}

#[test]
fn docker_requires_binary() {
    let entry = find("docker").unwrap();
    assert_eq!(entry.install_check, InstallCheck::Binary("docker"));
}

#[test]
fn postgres_requires_env_var() {
    let entry = find("postgres").unwrap();
    assert_eq!(
        entry.install_check,
        InstallCheck::NpxWithEnv("POSTGRES_URL")
    );
}

// ── Version pinning tests ──────────────────────────────────────────

#[test]
fn version_pinned_in_args() {
    // All npx-based servers should have a pinned version in their args (no @latest)
    for entry in CATALOG {
        if entry.command == "npx" {
            let pkg_arg = entry
                .args
                .iter()
                .find(|a| a.starts_with('@') || a.contains('/'));
            if let Some(arg) = pkg_arg {
                assert!(
                    arg.contains('@'),
                    "server {} args should contain versioned package, got: {}",
                    entry.key,
                    arg
                );
            }
        }
    }
}

#[test]
fn context7_has_pinned_version() {
    let entry = find("context7").unwrap();
    assert_eq!(entry.version, "0.0.19");
    // Verify the version appears in args
    assert!(entry.args.iter().any(|a| a.contains("0.0.19")));
}

#[test]
fn cloud_category_exists() {
    let entry = find("aws").unwrap();
    assert!(entry.categories.contains(&Category::Cloud));
    assert_eq!(format!("{}", Category::Cloud), "cloud");
}

#[test]
fn install_check_binary_with_env_distinguishes_credentials_from_installation() {
    let check = InstallCheck::BinaryWithEnv("fixture-tool", "FIXTURE_KEY");
    assert_eq!(
        check.check_with(|_| true, |_| false),
        InstallStatus::MissingEnvVar("FIXTURE_KEY")
    );
    assert_eq!(
        check.check_with(|_| true, |_| true),
        InstallStatus::Available
    );
}
