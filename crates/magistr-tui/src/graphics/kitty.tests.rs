use proptest::prelude::*;

use super::*;
use magistr_cockpit::base64;

fn id(raw: u32) -> ImageId {
    ImageId::new(raw).expect("non-zero image id")
}

fn placement(raw: u32) -> PlacementId {
    PlacementId::new(raw).expect("non-zero placement id")
}

/// Split a byte stream into the APC commands it contains
/// (`ESC _ G … ESC \`), returning the control keys and payload of each.
fn commands(bytes: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8(bytes.to_vec()).expect("ascii");
    text.split("\x1b_G")
        .skip(1)
        .map(|rest| {
            let body = rest.strip_suffix("\x1b\\").expect("APC terminator");
            let (keys, payload) = body.split_once(';').unwrap_or((body, ""));
            (keys.to_owned(), payload.to_owned())
        })
        .collect()
}

fn has_key(keys: &str, key: &str) -> bool {
    keys.split(',').any(|k| k == key)
}

#[test]
fn the_query_is_the_spec_example_byte_for_byte() {
    let mut out = Vec::new();
    Kitty::new(Passthrough::Direct).query(&mut out);
    assert_eq!(out, b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\");
}

#[test]
fn the_query_is_wrapped_for_tmux() {
    let mut out = Vec::new();
    Kitty::new(Passthrough::Tmux).query(&mut out);
    assert_eq!(
        out,
        b"\x1bPtmux;\x1b\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\x1b\\\x1b\\"
    );
}

#[test]
fn a_small_rgba_image_is_one_chunk_with_every_key() {
    let pixels = [255u8, 0, 0, 255, 0, 255, 0, 255];
    let mut out = Vec::new();
    Kitty::new(Passthrough::Direct)
        .transmit(&mut out, id(7), PixelFormat::Rgba, 2, 1, &pixels)
        .expect("2x1 rgba");
    let mut expected_payload = Vec::new();
    base64::encode(&pixels, &mut expected_payload);
    let expected = [
        b"\x1b_Ga=t,q=2,f=32,s=2,v=1,i=7,m=0;".as_slice(),
        &expected_payload,
        b"\x1b\\",
    ]
    .concat();
    assert_eq!(out, expected);
}

#[test]
fn rgb_uses_format_24_and_three_bytes_per_pixel() {
    let pixels = [1u8, 2, 3, 4, 5, 6, 7, 8, 9];
    let mut out = Vec::new();
    Kitty::new(Passthrough::Direct)
        .transmit(&mut out, id(1), PixelFormat::Rgb, 3, 1, &pixels)
        .expect("3x1 rgb");
    let cmds = commands(&out);
    assert_eq!(cmds.len(), 1);
    assert!(has_key(&cmds[0].0, "f=24"), "{}", cmds[0].0);
    assert!(has_key(&cmds[0].0, "s=3"));
    assert!(has_key(&cmds[0].0, "v=1"));
}

#[test]
fn a_large_image_is_chunked_at_4096_base64_bytes() {
    // 40x12 cells at 10x20 px = 400x240 RGBA = 384_000 bytes = 512_000 b64.
    let pixels = vec![0x5Au8; 400 * 240 * 4];
    let mut out = Vec::new();
    Kitty::new(Passthrough::Direct)
        .transmit(&mut out, id(9), PixelFormat::Rgba, 400, 240, &pixels)
        .expect("400x240 rgba");
    let cmds = commands(&out);
    assert_eq!(cmds.len(), 512_000 / 4096);
    let (first_keys, _) = &cmds[0];
    for key in ["a=t", "q=2", "f=32", "s=400", "v=240", "i=9", "m=1"] {
        assert!(
            has_key(first_keys, key),
            "first chunk lacks {key}: {first_keys}"
        );
    }
    for (keys, payload) in &cmds[1..cmds.len() - 1] {
        assert_eq!(keys, "m=1,q=2");
        assert_eq!(payload.len(), 4096);
    }
    assert_eq!(cmds[cmds.len() - 1].0, "m=0,q=2");
    let mut reassembled = String::new();
    for (_, payload) in &cmds {
        reassembled.push_str(payload);
    }
    let mut expected = Vec::new();
    base64::encode(&pixels, &mut expected);
    assert_eq!(reassembled.as_bytes(), expected.as_slice());
}

#[test]
fn transmit_refuses_a_pixel_buffer_of_the_wrong_size() {
    let mut out = Vec::new();
    let kitty = Kitty::new(Passthrough::Direct);
    let error = kitty
        .transmit(&mut out, id(1), PixelFormat::Rgba, 2, 2, &[0; 15])
        .expect_err("15 bytes for 2x2 rgba");
    assert_eq!(
        error,
        GraphicsError::PixelCount {
            expected: 16,
            actual: 15
        }
    );
    assert!(out.is_empty(), "nothing is written on error");
    let error = kitty
        .transmit(&mut out, id(1), PixelFormat::Rgb, 0, 4, &[])
        .expect_err("empty image");
    assert_eq!(error, GraphicsError::EmptyImage);
    assert!(error.to_string().contains("empty"));
}

#[test]
fn place_names_image_placement_rect_and_z_and_keeps_the_cursor() {
    let mut out = Vec::new();
    Kitty::new(Passthrough::Direct).place(
        &mut out,
        id(7),
        placement(2),
        40,
        12,
        Kitty::Z_UNDER_TEXT,
    );
    assert_eq!(out, b"\x1b_Ga=p,q=2,i=7,p=2,c=40,r=12,z=-1,C=1\x1b\\");
}

#[test]
fn under_background_z_is_below_the_spec_threshold() {
    const { assert!(Kitty::Z_UNDER_BACKGROUND < -1_073_741_824) };
    let mut out = Vec::new();
    Kitty::new(Passthrough::Direct).place(
        &mut out,
        id(1),
        placement(1),
        1,
        1,
        Kitty::Z_UNDER_BACKGROUND,
    );
    assert!(
        String::from_utf8_lossy(&out).contains("z=-1073741825"),
        "{}",
        String::from_utf8_lossy(&out)
    );
}

#[test]
fn deleting_a_placement_keeps_the_image_data_and_deleting_an_image_frees_it() {
    let kitty = Kitty::new(Passthrough::Direct);
    let mut out = Vec::new();
    kitty.delete_placement(&mut out, id(7), placement(2));
    assert_eq!(out, b"\x1b_Ga=d,q=2,d=i,i=7,p=2\x1b\\");
    out.clear();
    kitty.delete_image(&mut out, id(7));
    assert_eq!(out, b"\x1b_Ga=d,q=2,d=I,i=7\x1b\\");
}

#[test]
fn every_command_is_wrapped_for_tmux() {
    let kitty = Kitty::new(Passthrough::Tmux);
    let mut out = Vec::new();
    kitty
        .transmit(&mut out, id(3), PixelFormat::Rgb, 1, 1, &[9, 9, 9])
        .expect("1x1");
    kitty.place(&mut out, id(3), placement(1), 1, 1, 0);
    kitty.delete_placement(&mut out, id(3), placement(1));
    kitty.delete_image(&mut out, id(3));
    let text = String::from_utf8(out).expect("ascii");
    let wrappers = text.matches("\x1bPtmux;").count();
    assert_eq!(wrappers, 4);
    // Inside a wrapper every ESC is doubled: no lone ESC _ or ESC \ remains.
    let inner: Vec<&str> = text.split("\x1bPtmux;").skip(1).collect();
    for seq in inner {
        let seq = seq.strip_suffix("\x1b\\").expect("DCS terminator");
        let mut bytes = seq.bytes();
        while let Some(byte) = bytes.next() {
            if byte == 0x1b {
                assert_eq!(bytes.next(), Some(0x1b), "lone ESC in {seq:?}");
            }
        }
        assert!(seq.starts_with("\x1b\x1b_G"), "{seq:?}");
        assert!(seq.ends_with("\x1b\x1b\\"), "{seq:?}");
    }
}

proptest! {
    #[test]
    fn chunks_are_multiples_of_four_and_reassemble(
        width in 1u16..64,
        height in 1u16..64,
        seed in any::<u8>(),
    ) {
        let pixels: Vec<u8> = (0..usize::from(width) * usize::from(height) * 4)
            .map(|i| (i as u8).wrapping_mul(seed))
            .collect();
        let mut out = Vec::new();
        Kitty::new(Passthrough::Direct)
            .transmit(&mut out, id(5), PixelFormat::Rgba, width, height, &pixels)
            .expect("sized to fit");
        let cmds = commands(&out);
        let mut expected = Vec::new();
        base64::encode(&pixels, &mut expected);
        prop_assert_eq!(cmds.len(), expected.len().div_ceil(Kitty::CHUNK).max(1));
        for (keys, payload) in &cmds[..cmds.len() - 1] {
            prop_assert_eq!(payload.len() % 4, 0);
            prop_assert!(payload.len() <= Kitty::CHUNK);
            prop_assert!(has_key(keys, "m=1"));
        }
        prop_assert!(has_key(&cmds[cmds.len() - 1].0, "m=0"));
        let joined: String = cmds.iter().map(|(_, p)| p.as_str()).collect();
        prop_assert_eq!(joined.into_bytes(), expected);
    }
}

/// RFC 4648 decode, test-only: the inverse of `base64::encode`.
fn base64_decode(text: &str) -> Vec<u8> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let value = |c: u8| ALPHABET.iter().position(|&a| a == c).expect("alphabet") as u32;
    let mut out = Vec::new();
    for quad in text.as_bytes().chunks(4) {
        let pad = quad.iter().filter(|&&c| c == b'=').count();
        let n = quad
            .iter()
            .map(|&c| if c == b'=' { 0 } else { value(c) })
            .fold(0u32, |acc, v| (acc << 6) | v);
        let bytes = n.to_be_bytes();
        out.extend_from_slice(&bytes[1..4 - pad]);
    }
    out
}

#[test]
fn a_zlib_transmit_carries_o_z_and_inflates_to_the_pixels() {
    use std::io::Read as _;
    let pixels: Vec<u8> = (0..30 * 10 * 3).map(|i| (i % 7) as u8).collect();
    let mut deflated = Vec::new();
    flate2::read::ZlibEncoder::new(pixels.as_slice(), flate2::Compression::fast())
        .read_to_end(&mut deflated)
        .expect("deflate");
    let mut out = Vec::new();
    Kitty::new(Passthrough::Direct)
        .transmit_zlib(&mut out, id(4), PixelFormat::Rgb, 30, 10, &deflated)
        .expect("30x10 rgb zlib");
    let cmds = commands(&out);
    let first = &cmds[0].0;
    for key in ["a=t", "q=2", "f=24", "s=30", "v=10", "i=4", "o=z"] {
        assert!(has_key(first, key), "first chunk lacks {key}: {first}");
    }
    let joined: String = cmds.iter().map(|(_, p)| p.as_str()).collect();
    let mut inflated = Vec::new();
    flate2::read::ZlibDecoder::new(base64_decode(&joined).as_slice())
        .read_to_end(&mut inflated)
        .expect("inflate");
    assert_eq!(inflated, pixels);
}

#[test]
fn a_zlib_transmit_still_refuses_an_empty_image() {
    let mut out = Vec::new();
    let error = Kitty::new(Passthrough::Direct)
        .transmit_zlib(&mut out, id(1), PixelFormat::Rgb, 0, 1, &[1, 2, 3])
        .expect_err("empty");
    assert_eq!(error, GraphicsError::EmptyImage);
    assert!(out.is_empty());
}
