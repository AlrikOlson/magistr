/// Fuzzy match: check if all characters in `query` appear in `text` in order.
///
/// Returns the matched character positions if successful, `None` otherwise.
/// Case-insensitive matching.
///
/// Used by command palette, completion popup, and any future fuzzy-filtered lists.
pub fn fuzzy_match(text: &str, query: &str) -> Option<Vec<usize>> {
    let text_lower = text.to_lowercase();
    let text_chars: Vec<char> = text_lower.chars().collect();
    let query_chars: Vec<char> = query.to_lowercase().chars().collect();

    let mut positions = Vec::with_capacity(query_chars.len());
    let mut text_idx = 0;

    for &qc in &query_chars {
        let mut found = false;
        while text_idx < text_chars.len() {
            if text_chars[text_idx] == qc {
                positions.push(text_idx);
                text_idx += 1;
                found = true;
                break;
            }
            text_idx += 1;
        }
        if !found {
            return None;
        }
    }

    Some(positions)
}

#[cfg(test)]
#[path = "fuzzy.tests.rs"]
mod tests;
