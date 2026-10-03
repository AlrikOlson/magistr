use super::*;
use crate::mcp_tools::owner_policy::Policy;

struct BrokenLink;
impl BridgeLink for BrokenLink {
    type Options = ();
    async fn open(_: &()) -> Result<Self, String> {
        Err("no owner".to_owned())
    }
    fn generation(&self) -> u64 {
        1
    }
    fn is_embedded(&self) -> bool {
        false
    }
    async fn next_line(&mut self) -> Option<String> {
        None
    }
    async fn send(&mut self, _: &str) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "owner exited",
        ))
    }
    async fn half_close(&mut self) -> bool {
        true
    }
    async fn close(self) -> eyre::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn failed_send_answers_each_request_once_without_replaying_the_change() {
    let mut bridge = Bridge::new((), "test".to_owned(), Policy, Ok(BrokenLink));
    let request = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
        "params":{"name":"ship_record","arguments":{"description":"ambiguous change"}}});
    let mut output = Vec::new();
    bridge
        .on_client(&request.to_string(), &mut output)
        .await
        .expect("refusal");
    let text = String::from_utf8(output).unwrap();
    let answers: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        answers.len(),
        1,
        "one terminal answer despite a failed send"
    );
    assert_eq!(answers[0]["id"], 1);
    assert_eq!(
        answers[0]["result"]["structuredContent"]["effect"],
        "unknown"
    );
    assert!(!bridge.session.has_outstanding());
    assert!(bridge.link.is_none());
}
