/// Sanitize a project name into a slug suitable for state prefix.
///
/// Converts to lowercase, replaces non-ASCII-alphanumeric characters with dashes,
/// collapses consecutive dashes, and trims leading/trailing dashes.
#[must_use]
pub fn sanitize_prefix(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();

    // Collapse multiple consecutive dashes and trim leading/trailing dashes.
    let mut result = String::new();
    let mut prev_dash = false;
    for c in slug.chars() {
        if c == '-' {
            if !prev_dash && !result.is_empty() {
                result.push('-');
            }
            prev_dash = true;
        } else {
            result.push(c);
            prev_dash = false;
        }
    }
    result.trim_end_matches('-').to_string()
}

#[cfg(test)]
#[path = "slug.tests.rs"]
mod tests;
