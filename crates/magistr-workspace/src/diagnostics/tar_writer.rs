//! Minimal POSIX ustar writer for the doctor bundle. Deliberately narrow:
//! flat member names under 100 bytes, regular files only, no pax
//! extensions, no links. Any `tar -xzf` opens the result; the tests parse
//! the headers with an independent reader to pin conformance.
const BLOCK: usize = 512;

pub(super) struct TarMember {
    /// Full member path, e.g. `workbench-doctor/manifest.json`. ≤100 bytes.
    pub(super) name: String,
    pub(super) data: Vec<u8>,
}

/// Zero-padded octal with a trailing NUL, per the ustar numeric fields.
fn octal(value: u64, width: usize) -> Vec<u8> {
    let mut field = format!("{value:0width$o}", width = width - 1).into_bytes();
    field.push(0);
    field
}

fn ascii(text: &str, width: usize) -> std::io::Result<Vec<u8>> {
    if text.len() > width {
        return Err(std::io::Error::other(format!(
            "tar field overflow: {text:?} exceeds {width} bytes"
        )));
    }
    let mut field = text.as_bytes().to_vec();
    field.resize(width, 0);
    Ok(field)
}

fn header(name: &str, size: usize, mtime_seconds: u64) -> std::io::Result<[u8; BLOCK]> {
    if name.len() > 100 {
        return Err(std::io::Error::other(format!(
            "tar member name exceeds 100 bytes: {name}"
        )));
    }
    let mut block = [0_u8; BLOCK];
    let mut put =
        |offset: usize, bytes: &[u8]| block[offset..offset + bytes.len()].copy_from_slice(bytes);
    put(0, &ascii(name, 100)?);
    put(100, &ascii("0000644", 8)?);
    put(108, &octal(0, 8));
    put(116, &octal(0, 8));
    put(124, &octal(size as u64, 12));
    put(136, &octal(mtime_seconds, 12));
    // chksum is computed over the header with the field set to 8 spaces.
    put(148, &[0x20; 8]);
    put(156, b"0");
    put(257, &ascii("ustar", 6)?);
    put(263, b"00");
    let sum: u64 = block.iter().map(|byte| u64::from(*byte)).sum();
    // "%06o\0 ": six octal digits, NUL, space.
    let mut checksum = format!("{sum:06o}").into_bytes();
    checksum.push(0);
    checksum.push(0x20);
    block[148..156].copy_from_slice(&checksum);
    Ok(block)
}

/// Serialize members into a ustar archive (uncompressed).
pub(super) fn tar_archive(members: &[TarMember], mtime_seconds: u64) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    for member in members {
        out.extend_from_slice(&header(&member.name, member.data.len(), mtime_seconds)?);
        out.extend_from_slice(&member.data);
        let pad = (BLOCK - member.data.len() % BLOCK) % BLOCK;
        out.resize(out.len() + pad, 0);
    }
    // End-of-archive: two zero blocks.
    out.resize(out.len() + BLOCK * 2, 0);
    Ok(out)
}
