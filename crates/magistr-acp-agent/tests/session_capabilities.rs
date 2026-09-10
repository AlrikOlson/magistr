//! Provider promises and refusal before a session or turn can lose information.

mod fake_client;
#[path = "../../magistr-cli/tests/support/in_env.rs"]
mod in_env;

use agent_client_protocol::schema::v1::{
    ContentBlock, LoadSessionRequest, McpServer, McpServerStdio, NewSessionRequest, PromptRequest,
    SessionId,
};
use fake_client::{
    PermissionAnswer, initialize, new_session, prompt_and_wait, with_client, with_provider,
};
use magistr_acp_agent::{ApiProvider, OpenAi, Provider, SessionStart};

#[tokio::test]
async fn api_sessions_cannot_be_restored_in_a_fresh_process() {
    const SESSION_ENV: &str = "MAGISTR_TEST_PREVIOUS_API_SESSION";
    let provider = ApiProvider::new(OpenAi::new("test", "http://127.0.0.1:1"));
    let session_id = match std::env::var(SESSION_ENV) {
        Ok(id) => SessionId::new(id),
        Err(_) => {
            let opened = provider
                .start(SessionStart {
                    session_id: SessionId::new("previous-process-session"),
                    cwd: std::env::temp_dir(),
                    mcp_servers: Vec::new(),
                    resume: false,
                    model: None,
                    effort: None,
                })
                .await
                .expect("ordinary API session");
            drop(provider);
            let id = opened.session_id.to_string();
            assert!(!in_env::in_env(&[(
                SESSION_ENV,
                Some(std::ffi::OsStr::new(&id))
            )]));
            return;
        }
    };
    let error = provider
        .start(SessionStart {
            session_id: session_id.clone(),
            cwd: std::env::temp_dir(),
            mcp_servers: Vec::new(),
            resume: true,
            model: None,
            effort: None,
        })
        .await
        .expect_err("ephemeral history cannot be restored");
    assert!(error.to_string().contains("durable"), "{error}");
    let (_, observed) = with_provider(provider, PermissionAnswer::Cancel, async move |cx, _| {
        let init = initialize(&cx).await?;
        assert!(!init.agent_capabilities.load_session);
        let error = cx
            .send_request(LoadSessionRequest::new(
                session_id.clone(),
                std::env::temp_dir(),
            ))
            .block_task()
            .await
            .expect_err("load must refuse even when the client ignores capabilities");
        assert!(
            error
                .data
                .as_ref()
                .is_some_and(|value| value.to_string().contains("durable"))
        );
        prompt_and_wait(&cx, &session_id, "continue")
            .await
            .expect_err("failed load did not register a fresh empty session");
        Ok(())
    })
    .await;
    assert!(observed.updates.lock().expect("updates").is_empty());
}

#[tokio::test]
async fn unsupported_content_is_refused_before_the_provider_sees_a_partial_prompt() {
    let (_, observed) = with_client(PermissionAnswer::Cancel, async |cx, _| {
        let init = initialize(&cx).await?;
        assert!(!init.agent_capabilities.prompt_capabilities.embedded_context);
        let session = new_session(&cx).await?;
        let image: ContentBlock = serde_json::from_value(serde_json::json!({
            "type":"image", "data":"AA==", "mimeType":"image/png"
        }))
        .expect("image block");
        cx.send_request(PromptRequest::new(session.session_id, vec![image]))
            .block_task()
            .await
            .expect_err("unsupported image cannot become an empty turn");
        Ok(())
    })
    .await;
    assert!(observed.updates.lock().expect("updates").is_empty());
}

#[tokio::test]
async fn chat_api_refuses_tools_and_effort_without_claiming_an_open_session() {
    let provider = ApiProvider::new(OpenAi::new("test", "http://127.0.0.1:1"));
    let (_, observed) = with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
        let init = initialize(&cx).await?;
        let meta = init.meta.expect("capabilities");
        assert_eq!(meta["magistr"]["capabilities"]["tools"], false);
        assert_eq!(
            meta["magistr"]["capabilities"]["ephemeral_continuation"],
            true
        );
        assert_eq!(meta["magistr"]["session_options"]["effort"], false);
        let tools =
            NewSessionRequest::new(std::env::temp_dir()).mcp_servers(vec![McpServer::Stdio(
                McpServerStdio::new("tools", "/fixture/tool"),
            )]);
        cx.send_request(tools)
            .block_task()
            .await
            .expect_err("chat APIs have no tool harness");
        for session in [
            serde_json::json!({"effort":"high"}),
            serde_json::json!({"model":"unknown"}),
            serde_json::json!({"effort":42}),
        ] {
            let meta: agent_client_protocol::schema::v1::Meta =
                serde_json::from_value(serde_json::json!({"magistr":{"session":session}}))
                    .expect("metadata");
            cx.send_request(NewSessionRequest::new(std::env::temp_dir()).meta(meta))
                .block_task()
                .await
                .expect_err("unsupported session option");
        }
        Ok(())
    })
    .await;
    assert!(observed.updates.lock().expect("updates").is_empty());
}
