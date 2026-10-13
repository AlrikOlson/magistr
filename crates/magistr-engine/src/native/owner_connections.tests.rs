use super::*;

use crate::native::{CallerIdentity, OwnerClient, OwnerEndpoint};

#[tokio::test]
async fn workspace_connection_admission_counts_every_live_session() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let acceptor: McpAcceptor = Arc::new(|stream: LocalStream, cancel, _context| {
        Box::pin(async move {
            cancel.cancelled().await;
            drop(stream);
        })
    });
    let endpoint = OwnerEndpoint::serve_workspace(tmp.path(), 1, acceptor)
        .await
        .expect("endpoint");
    let mut attached = Vec::new();
    for _ in 0..OwnerConnections::CONNECTION_LIMIT {
        let (_, stream) = OwnerClient::attach_workspace(
            tmp.path(),
            CallerIdentity::this_process("limit-test"),
            Duration::from_secs(2),
        )
        .await
        .expect("admitted session");
        attached.push(stream);
    }
    let excess = OwnerClient::attach_workspace(
        tmp.path(),
        CallerIdentity::this_process("excess-client"),
        Duration::from_secs(2),
    )
    .await;
    assert!(
        excess.is_err(),
        "the connection ceiling includes established MCP sessions"
    );
    endpoint.shutdown().await.expect("joined bounded clients");
    drop(attached);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_attachment_uses_the_owner_contract_across_rebuilds() {
    use crate::native::{
        BuildIdentity, NativeConfig, OWNER_PROTOCOL_VERSION, ProjectAuthority, WriterRole,
    };
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let tmp = tempfile::tempdir().expect("tempdir");
    let acceptor: McpAcceptor = Arc::new(|mut stream: LocalStream, _cancel, _context| {
        Box::pin(async move {
            let _ = stream.write_all(b"served-by-owner\n").await;
        })
    });
    let workspace = OwnerEndpoint::serve_workspace(tmp.path(), 1, acceptor.clone())
        .await
        .expect("workspace endpoint");
    let config =
        NativeConfig::from_lookup(tmp.path(), |_| None).with_data_dir(tmp.path().join("data"));
    let engines = ProjectAuthority::admit(&config, WriterRole::Service)
        .expect("writer")
        .open();
    let workflow = OwnerEndpoint::serve_with(engines, Some(acceptor))
        .await
        .expect("workflow endpoint");
    let build = BuildIdentity::this_build();
    let rebuilt = BuildIdentity::new(
        build.package(),
        "ffffffffffffffff",
        if build.profile() == "debug" {
            "release"
        } else {
            "debug"
        },
        build.target(),
    );
    let identities = [
        (
            workspace.path(),
            serde_json::json!({"kind":"workspace","root":tmp.path().canonicalize().unwrap()}),
        ),
        (
            workflow.path(),
            serde_json::json!({"kind":"workflow","project_id":config.resolve_project_id(),"role":"service"}),
        ),
    ];
    for (path, identity) in identities {
        let mut stream = tokio::net::UnixStream::connect(path).await.expect("client");
        let hello = serde_json::json!({"protocol":OWNER_PROTOCOL_VERSION, "build":rebuilt,
            "identity":identity,"context":config.execution_context(),"attach":"mcp","caller":{"pid":std::process::id(),"surface":"rebuilt-proxy"}});
        stream
            .write_all(format!("{hello}\n").as_bytes())
            .await
            .expect("hello");
        let mut lines = BufReader::new(stream).lines();
        let greeting: serde_json::Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(greeting["greeting"], "welcome", "{greeting}");
        assert_eq!(
            lines.next_line().await.unwrap().as_deref(),
            Some("served-by-owner")
        );
    }
    workflow.shutdown().await.expect("workflow stopped");
    workspace.shutdown().await.expect("workspace stopped");
}
