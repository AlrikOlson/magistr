use super::*;

#[test]
fn messages_carry_the_numbers() {
    let error = GraphicsError::PixelCount {
        expected: 16,
        actual: 15,
    };
    assert_eq!(
        error.to_string(),
        "pixel buffer holds 15 bytes, the image needs 16"
    );
    assert!(GraphicsError::EmptyImage.to_string().contains("empty"));
    let boxed: Box<dyn std::error::Error> = Box::new(error);
    assert!(boxed.source().is_none());
}
