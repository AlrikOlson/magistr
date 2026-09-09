//! The kitty graphics protocol as bytes.
//!
//! Every command is an APC: `ESC _ G <key=value,…> ; <payload> ESC \`.
//! Image data travels base64-encoded in chunks of at most 4096 bytes,
//! `m=1` on every chunk but the last; only the first chunk carries the
//! image keys. Placements are addressed by image id and placement id so a
//! frame can replace or delete exactly one on-screen instance. `q=2` on
//! every command keeps the terminal from answering — the cockpit's input
//! thread must never see graphics replies mid-session — except on the
//! query, whose whole point is the reply.

use super::error::GraphicsError;
use super::image_id::ImageId;
use super::passthrough::Passthrough;
use super::placement_id::PlacementId;
use crate::render::encode::push_number;
use magistr_cockpit::base64;
use magistr_cockpit::pixel::PixelFormat;

/// The keys of a transmit's first chunk.
#[derive(Clone, Copy)]
struct Header {
    id: ImageId,
    format: PixelFormat,
    width: u16,
    height: u16,
    zlib: bool,
}

/// Writes kitty graphics commands, wrapped for tmux when required.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kitty {
    passthrough: Passthrough,
}

impl Kitty {
    /// Largest base64 payload per chunk the protocol allows.
    pub const CHUNK: usize = 4096;
    /// A z-index that draws the placement under the text of its cells.
    pub const Z_UNDER_TEXT: i32 = -1;
    /// A z-index that draws the placement under the cells' background
    /// colour too (the protocol's threshold is -1_073_741_824).
    pub const Z_UNDER_BACKGROUND: i32 = -1_073_741_825;

    /// An encoder for a terminal reached through `passthrough`.
    #[must_use]
    pub const fn new(passthrough: Passthrough) -> Self {
        Self { passthrough }
    }

    /// The support query from the protocol's own example: a 1×1 RGB image
    /// under id 31, action `q`. A terminal that speaks the protocol answers
    /// `ESC _ G i=31;OK ESC \`; any other terminal ignores the APC.
    pub fn query(&self, out: &mut Vec<u8>) {
        self.passthrough
            .wrap(b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\", out);
    }

    /// Transmit (`a=t`) a `width × height` image under `id` without
    /// displaying it, replacing any image already stored under that id.
    ///
    /// `pixels` is row-major in `format`'s layout. Nothing is written when
    /// the buffer does not match the dimensions.
    pub fn transmit(
        &self,
        out: &mut Vec<u8>,
        id: ImageId,
        format: PixelFormat,
        width: u16,
        height: u16,
        pixels: &[u8],
    ) -> Result<(), GraphicsError> {
        if width == 0 || height == 0 {
            return Err(GraphicsError::EmptyImage);
        }
        let expected = usize::from(width) * usize::from(height) * format.bytes_per_pixel();
        if pixels.len() != expected {
            return Err(GraphicsError::PixelCount {
                expected,
                actual: pixels.len(),
            });
        }
        self.send(
            out,
            Header {
                id,
                format,
                width,
                height,
                zlib: false,
            },
            pixels,
        );
        Ok(())
    }

    /// Transmit (`a=t`, `o=z`) a `width × height` image whose `format`
    /// pixels were deflated into a zlib stream (RFC 1950) — `deflated` is
    /// what the terminal inflates. Only the dimensions can be checked here.
    pub fn transmit_zlib(
        &self,
        out: &mut Vec<u8>,
        id: ImageId,
        format: PixelFormat,
        width: u16,
        height: u16,
        deflated: &[u8],
    ) -> Result<(), GraphicsError> {
        if width == 0 || height == 0 {
            return Err(GraphicsError::EmptyImage);
        }
        self.send(
            out,
            Header {
                id,
                format,
                width,
                height,
                zlib: true,
            },
            deflated,
        );
        Ok(())
    }

    /// Base64 `payload` and write it as the chunks of one transmit command.
    fn send(&self, out: &mut Vec<u8>, header: Header, payload: &[u8]) {
        let Header {
            id,
            format,
            width,
            height,
            zlib,
        } = header;
        let mut encoded = Vec::with_capacity(payload.len().div_ceil(3) * 4);
        base64::encode(payload, &mut encoded);
        let chunks = encoded.chunks(Self::CHUNK);
        let last = chunks.len().saturating_sub(1);
        for (index, chunk) in chunks.enumerate() {
            let more = if index < last { b'1' } else { b'0' };
            self.begin(out);
            if index == 0 {
                out.extend_from_slice(b"a=t,q=2,f=");
                push_number(out, format_code(format));
                out.extend_from_slice(b",s=");
                push_number(out, u32::from(width));
                out.extend_from_slice(b",v=");
                push_number(out, u32::from(height));
                out.extend_from_slice(b",i=");
                push_number(out, id.raw());
                if zlib {
                    out.extend_from_slice(b",o=z");
                }
                out.extend_from_slice(b",m=");
                out.push(more);
            } else {
                out.extend_from_slice(b"m=");
                out.push(more);
                out.extend_from_slice(b",q=2");
            }
            out.push(b';');
            out.extend_from_slice(chunk);
            self.end(out);
        }
    }

    /// Place (`a=p`) the stored image `id` as `placement` at the cursor,
    /// scaled into `cols × rows` cells at z-index `z`, without moving the
    /// cursor (`C=1`).
    pub fn place(
        &self,
        out: &mut Vec<u8>,
        id: ImageId,
        placement: PlacementId,
        cols: u16,
        rows: u16,
        z: i32,
    ) {
        self.begin(out);
        out.extend_from_slice(b"a=p,q=2,i=");
        push_number(out, id.raw());
        out.extend_from_slice(b",p=");
        push_number(out, placement.raw());
        out.extend_from_slice(b",c=");
        push_number(out, u32::from(cols));
        out.extend_from_slice(b",r=");
        push_number(out, u32::from(rows));
        out.extend_from_slice(b",z=");
        if z < 0 {
            out.push(b'-');
        }
        push_number(out, z.unsigned_abs());
        out.extend_from_slice(b",C=1");
        self.end(out);
    }

    /// Delete one placement (`d=i`), keeping the image data so it can be
    /// placed again.
    pub fn delete_placement(&self, out: &mut Vec<u8>, id: ImageId, placement: PlacementId) {
        self.begin(out);
        out.extend_from_slice(b"a=d,q=2,d=i,i=");
        push_number(out, id.raw());
        out.extend_from_slice(b",p=");
        push_number(out, placement.raw());
        self.end(out);
    }

    /// Delete every placement of `id` and free its data (`d=I`).
    pub fn delete_image(&self, out: &mut Vec<u8>, id: ImageId) {
        self.begin(out);
        out.extend_from_slice(b"a=d,q=2,d=I,i=");
        push_number(out, id.raw());
        self.end(out);
    }

    /// Open the (possibly wrapped) APC: `ESC _ G`.
    fn begin(&self, out: &mut Vec<u8>) {
        self.passthrough.open(out);
        self.passthrough.esc(out);
        out.extend_from_slice(b"_G");
    }

    /// Close the APC (`ESC \`) and the envelope.
    fn end(&self, out: &mut Vec<u8>) {
        self.passthrough.esc(out);
        out.push(b'\\');
        self.passthrough.close(out);
    }
}

/// The value of kitty's `f=` key for a payload laid out as `format`:
/// 24 for RGB, 32 for straight-alpha RGBA.
const fn format_code(format: PixelFormat) -> u32 {
    match format {
        PixelFormat::Rgb => 24,
        PixelFormat::Rgba => 32,
    }
}

#[cfg(test)]
#[path = "kitty.tests.rs"]
mod tests;
