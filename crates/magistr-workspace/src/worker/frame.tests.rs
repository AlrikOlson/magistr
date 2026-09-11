use super::*;
#[tokio::test]
async fn rejects_oversized_prefix_without_reading_payload() {
    let bytes = ((MAX_FRAME_BYTES + 1) as u32).to_le_bytes();
    assert!(matches!(
        Frame::read(&mut bytes.as_slice()).await,
        Err(WorkerError::Protocol(_))
    ));
}
#[tokio::test]
async fn checks_version_and_truncated_payload() {
    for (payload, version_error) in [
        (br#"{"v":2,"kind":"ready"}"#.as_slice(), true),
        (b"{".as_slice(), false),
    ] {
        let mut bytes = (payload.len() as u32).to_le_bytes().to_vec();
        bytes.extend_from_slice(payload);
        let result = Frame::read(&mut bytes.as_slice()).await;
        if version_error {
            assert!(matches!(result, Err(WorkerError::UnsupportedVersion)));
        } else {
            assert!(matches!(result, Err(WorkerError::Protocol(_))));
        }
    }
    assert!(
        Frame::read(&mut [4, 0, 0, 0, b'{'].as_slice())
            .await
            .is_err()
    );
}
#[tokio::test]
async fn handles_fragmented_and_coalesced_frames() {
    let (mut writer, mut reader) = tokio::io::duplex(2);
    let task = tokio::spawn(async move {
        Frame::Ready { v: VERSION }
            .write(&mut writer)
            .await
            .expect("ready");
        Frame::Shutdown { v: VERSION }
            .write(&mut writer)
            .await
            .expect("shutdown");
    });
    assert!(matches!(
        Frame::read(&mut reader).await,
        Ok(Frame::Ready { .. })
    ));
    assert!(matches!(
        Frame::read(&mut reader).await,
        Ok(Frame::Shutdown { .. })
    ));
    task.await.expect("writer joined");
}
