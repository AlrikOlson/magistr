use alloc::vec::Vec;

/// Match `candidate` against a shell-style `pattern`, case-insensitively.
///
/// Supports `*` (any run of characters, including none) and `?` (exactly
/// one character); every other character is literal. A pattern with no
/// wildcard is a plain equality test. Deliberately tiny: policy patterns
/// match tool names, command lines, paths and model ids, none of which
/// need character classes or brace expansion, and the workspace carries
/// no glob dependency to spend on it.
///
/// Matching is anchored at both ends — a `protected_paths` entry like
/// `.magistr/team.json` must be written `*team.json` (or an absolute
/// path) to catch a path with a leading directory, which keeps a rule
/// from silently matching more than it says.
#[must_use]
pub fn glob_match(pattern: &str, candidate: &str) -> bool {
    let pat: Vec<char> = pattern.to_lowercase().chars().collect();
    let text: Vec<char> = candidate.to_lowercase().chars().collect();

    // Classic linear backtracking: remember the last `*` and the text
    // position it had consumed up to, and resume from there on a
    // mismatch. O(n·m) worst case, O(n+m) in practice.
    let (mut p, mut t) = (0usize, 0usize);
    let mut star: Option<usize> = None;
    let mut resume = 0usize;

    while t < text.len() {
        if p < pat.len() && (pat[p] == '?' || pat[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pat.len() && pat[p] == '*' {
            star = Some(p);
            resume = t;
            p += 1;
        } else if let Some(s) = star {
            p = s + 1;
            resume += 1;
            t = resume;
        } else {
            return false;
        }
    }

    pat[p..].iter().all(|c| *c == '*')
}

#[cfg(test)]
#[path = "glob_match.tests.rs"]
mod tests;
