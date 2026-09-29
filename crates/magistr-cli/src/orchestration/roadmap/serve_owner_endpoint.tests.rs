use super::super::admit_native_host::admit_native_host_with;
use super::*;

#[tokio::test]
async fn a_project_without_native_engines_serves_nothing() {
    assert!(
        serve_owner_endpoint(None)
            .await
            .expect("nothing to serve")
            .is_none()
    );
}

#[cfg(not(unix))]
#[tokio::test]
async fn a_host_off_unix_runs_without_an_endpoint() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join(".magistr")).expect("mkdir");
    std::fs::write(
        project.join(".magistr/project.json"),
        r#"{"project":{"name":"t","description":"","type":"cli"},"roadmap_backend":"think-and-ship"}"#,
    )
    .expect("project.json");
    let data_dir = tmp.path().join("data").display().to_string();
    let lookup = move |key: &str| (key == "THINK_AND_SHIP_DATA_DIR").then(|| data_dir.clone());
    let engines = admit_native_host_with(&project, &lookup)
        .expect("admitted")
        .expect("native engines");

    let endpoint = serve_owner_endpoint(Some(&engines))
        .await
        .expect("the loop starts without an endpoint");
    assert!(
        endpoint.is_none(),
        "no socket to attach a second cockpit to"
    );
}
