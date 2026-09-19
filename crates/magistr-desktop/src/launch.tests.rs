use super::*;
use std::cell::Cell;

#[tokio::test]
async fn window_failure_still_awaits_close_and_keeps_both_errors() {
    let closed = Cell::new(false);
    let shutdown = async {
        tokio::task::yield_now().await;
        closed.set(true);
        Err("close failed".to_owned())
    };
    let result = finish::<()>(Err(eyre::eyre!("window failed")), shutdown).await;
    assert!(
        closed.get(),
        "host cleanup must finish even when opening the window fails"
    );
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("window failed") && error.contains("close failed"),
        "{error}"
    );
}

#[tokio::test]
async fn successful_window_preserves_the_result_and_reports_cleanup_failure() {
    assert_eq!(finish(Ok(42), async { Ok(()) }).await.unwrap(), 42);
    assert_eq!(
        finish(Ok(42), async { Err("close failed".into()) })
            .await
            .unwrap_err()
            .to_string(),
        "close failed"
    );
}
