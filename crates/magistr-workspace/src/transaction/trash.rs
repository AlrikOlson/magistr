use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

/// The journal-backed trash below the protected state directory. Only the
/// pipeline's own deletion of an admitted target ever writes here.
pub(super) fn directory(root: &Path) -> io::Result<PathBuf> {
    let trash = root.join(".magistr/workspace").join("trash");
    fs::create_dir_all(&trash)?;
    Ok(trash)
}

/// A collision-safe trash name that keeps the original file name visible.
pub(super) fn entry_name(path: &Path) -> io::Result<OsString> {
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).map_err(io::Error::other)?;
    let mut name = OsString::from(format!("{:032x}-", u128::from_ne_bytes(random)));
    name.push(
        path.file_name()
            .ok_or_else(|| io::Error::other("no filename"))?,
    );
    Ok(name)
}
