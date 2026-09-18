//! File references explicitly included in a message, without reading any files.
use alloc::{borrow::ToOwned, string::String, vec::Vec};

/// Extract file names in first-occurrence order. The adapter decodes a quoted
/// name and reports its consumed byte length; JSON libraries stay at the edge.
pub fn prompt_file_references(
    text: &str,
    mut quoted: impl FnMut(&str) -> Result<(String, usize), String>,
) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = text[cursor..].find('@') {
        let at = cursor + offset;
        cursor = at + 1;
        if at > 0
            && !text[..at]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace)
        {
            continue;
        }
        let tail = &text[cursor..];
        let name = if tail.starts_with('"') {
            let (name, consumed) = quoted(tail)?;
            if consumed == 0 || !tail.is_char_boundary(consumed) {
                return Err("The file name could not be read. Choose it again.".to_owned());
            }
            cursor += consumed;
            name
        } else {
            let length = tail.find(char::is_whitespace).unwrap_or(tail.len());
            cursor += length;
            tail[..length].to_owned()
        };
        if !name.is_empty() && !files.contains(&name) {
            files.push(name);
        }
    }
    Ok(files)
}

#[cfg(test)]
#[path = "prompt_files.tests.rs"]
mod tests;
