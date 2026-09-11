use std::sync::OnceLock;

use regex::Regex;

/// Rewrite `import a.b.C;` (static included) lines in one java importer. The
/// import's dotted path must suffix-match the file's path at a path boundary;
/// java roots (src/main/java/…) sit above the package tree.
pub(super) fn rewrite_importer(content: &str, from: &str, to: &str) -> (String, usize) {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        Regex::new(r"(?m)(^\s*import\s+(?:static\s+)?)([\w.]+)(\s*;)").expect("static regex")
    });
    let Some(from_stem) = from.strip_suffix(".java") else {
        return (content.to_owned(), 0);
    };
    let Some(to_stem) = to.strip_suffix(".java") else {
        return (content.to_owned(), 0);
    };
    let from_dotted = from_stem.replace('/', ".");
    let mut count = 0;
    let rewritten = pattern.replace_all(content, |captures: &regex::Captures<'_>| {
        let whole = captures[0].to_owned();
        let head = &captures[1];
        let spec = &captures[2];
        let tail = &captures[3];
        let spec_as_path = spec.replace('.', "/");
        let matched = if from_dotted == spec
            || from_dotted.ends_with(&format!(".{spec}"))
            || from == format!("{spec_as_path}.java")
            || from.ends_with(&format!("/{spec_as_path}.java"))
        {
            spec.to_owned()
        } else if let Some((class, member)) = spec.rsplit_once('.') {
            // Static member import: `import static a.b.C.max;` — the class is
            // the specifier minus its last segment.
            let class_as_path = class.replace('.', "/");
            let class_file = format!("{class_as_path}.java");
            if from == class_file || from.ends_with(&format!("/{class_file}")) {
                let root = &from[..from.len() - class_file.len()];
                if !to.starts_with(root) {
                    return whole;
                }
                count += 1;
                let new_class = to_stem[root.len()..].replace('/', ".");
                return format!("{head}{new_class}.{member}{tail}");
            }
            return whole;
        } else {
            return whole;
        };
        let matched_file = format!("{}.java", matched.replace('.', "/"));
        let root = &from[..from.len() - matched_file.len()];
        if !to.starts_with(root) {
            return whole;
        }
        count += 1;
        format!("{head}{}{tail}", to_stem[root.len()..].replace('/', "."))
    });
    (rewritten.into_owned(), count)
}
