//! Owner-specific opening and settlement for the shared MCP relay.
use color_eyre::eyre;
use serde_json::Value;

pub(crate) trait BridgeLink: Send {
    type Options: Sync;
    fn open(options: &Self::Options) -> impl Future<Output = Result<Self, String>> + Send
    where
        Self: Sized;
    fn generation(&self) -> u64;
    fn is_embedded(&self) -> bool;
    fn next_line(&mut self) -> impl Future<Output = Option<String>> + Send;
    fn send(&mut self, line: &str) -> impl Future<Output = std::io::Result<()>> + Send;
    fn half_close(&mut self) -> impl Future<Output = bool> + Send;
    fn close(self) -> impl Future<Output = eyre::Result<()>> + Send;

    /// Only initialization is replayed. A tool whose result was lost never is.
    fn replay(
        &mut self,
        id: &Value,
        initialize: &Value,
        initialized: &Value,
    ) -> impl Future<Output = eyre::Result<Vec<String>>> + Send {
        async move {
            self.send(&initialize.to_string()).await?;
            let mut passthrough = Vec::new();
            let answered = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                while let Some(line) = self.next_line().await {
                    let message: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
                    if message.get("id") == Some(id) && message.get("method").is_none() {
                        return message.get("error").is_none();
                    }
                    passthrough.push(line);
                }
                false
            })
            .await;
            match answered {
                Ok(true) => {}
                Ok(false) => {
                    eyre::bail!("the owner refused the replayed initialize");
                }
                Err(_) => {
                    eyre::bail!("the owner did not answer the replayed initialize in time");
                }
            }
            self.send(&initialized.to_string()).await?;
            Ok(passthrough)
        }
    }
}
