use super::*;

use crate::native::{CallerIdentity, OwnerClient, OwnerEndpoint};

#[tokio::test]
async fn workspace_connection_admission_counts_every_live_session() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let acceptor: McpAcceptor = Arc::new(|stream: LocalStream, cancel| {
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
