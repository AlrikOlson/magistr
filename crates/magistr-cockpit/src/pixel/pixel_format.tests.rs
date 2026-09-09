use super::*;

#[test]
fn sizes_follow_the_layout() {
    assert_eq!(PixelFormat::Rgb.bytes_per_pixel(), 3);
    assert_eq!(PixelFormat::Rgba.bytes_per_pixel(), 4);
}
