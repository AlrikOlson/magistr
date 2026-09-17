//! Short chunk names — the label a canvas node wears.
//!
//! A chunk's `title` is a sentence stating a claim, and that is the house style:
//! measured across this project's own roadmap, zero of 519 titles are under 24
//! characters, the median is 90 and the longest is 160. A sentence is the right
//! thing to read once you have opened a chunk and the wrong thing to render 519
//! of on a canvas, so a chunk carries both — `title` keeps the claim, `name`
//! is what a node wears.
//!
//! # Where the budget comes from
//!
//! [`NAME_BUDGET`] is not a taste call. It is derived from the canvas map
//! level's mark size, and it is the number [`fits`] gates on.
//!
//! # Why the id is only a seed
//!
//! [`fn@derive`] exists so no chunk is ever nameless, not so names are computed.
//! The same argument the `group` field makes applies here: a derived-on-read
//! label can never be corrected, and names are one of the two things the
//! constraints document says need a machine *and* a human to maintain. So the
//! name is stored, seeded by this function, and overwritten the moment anyone
//! writes a better one.
//!
//! The derivation has to actively shorten rather than reformat. Reformatting the
//! id is the obvious approach and it does not work: 208 of this roadmap's 519
//! ids are themselves over budget (the longest is 47 characters), so humanizing
//! an id satisfies C8 for only 60% of the roadmap and silently fails the rest.

/// The most characters a node label may carry (constraint C8).
pub const NAME_BUDGET: usize = 24;

/// The most words a name may carry. A four-word fragment of a slug reads as a
/// truncated sentence, which is the thing being escaped; three is where a label
/// still reads as a label.
const MAX_WORDS: usize = 3;

/// Dropped from the front. An article carries no identity and spends characters
/// that the distinguishing words need.
const LEADING_NOISE: &[&str] = &["the", "a", "an"];

/// Dropped from the end. These are words a phrase leans on to reach the *next*
/// word, so when the next word is over budget they are left pointing at nothing
/// — "JWT secret is" says less than "JWT secret".
const TRAILING_NOISE: &[&str] = &[
    "is", "are", "was", "were", "be", "been", "the", "a", "an", "of", "on", "in", "into", "to",
    "and", "or", "for", "no", "not", "should", "shall", "can", "cannot", "its", "it", "that",
    "this", "these", "those", "from", "by", "with", "as", "at", "but", "so", "than", "then",
    "when", "while", "only", "just", "still", "does", "do", "did", "has", "have", "had", "must",
    "may", "might", "will", "would", "per", "via", "over", "under", "if", "unless",
];

/// Uppercased wholesale rather than title-cased. "Ai metering" and "Mcp entry"
/// read as typos; the acronym is the recognisable part of the label.
const ACRONYMS: &[&str] = &[
    "ai", "api", "cd", "ci", "cli", "cors", "cpu", "css", "csv", "db", "dns", "e2e", "env", "ftp",
    "gui", "html", "http", "https", "iac", "id", "imap", "io", "ipc", "json", "jwks", "jwt", "kv",
    "ldap", "llm", "md", "mcp", "mx", "npm", "oauth", "os", "otel", "pr", "rss", "saas", "sdk",
    "smtp", "spa", "sql", "sse", "ssh", "ssl", "tcp", "tls", "ttl", "tui", "udp", "ui", "url",
    "uuid", "ux", "wasm", "yaml",
];

/// Whether `name` is a usable node label: present, and within [`NAME_BUDGET`].
///
/// Counted in characters rather than bytes — the budget is about how much a
/// node can show, and a multi-byte character occupies one mark's worth of it.
#[must_use]
pub fn fits(name: &str) -> bool {
    let n = name.trim().chars().count();
    n > 0 && n <= NAME_BUDGET
}

/// How `name` fails [`fits`], as a phrase a diagnostic can print. `None` when it
/// passes.
#[must_use]
pub fn why_unfit(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Some("has no name".to_string());
    }
    let n = trimmed.chars().count();
    if n > NAME_BUDGET {
        return Some(format!(
            "name is {n} characters, over the {NAME_BUDGET}-character budget"
        ));
    }
    None
}

/// Seed a name from a chunk id. Total and deterministic: every id carrying any
/// content yields a name that satisfies [`fits`], including ids longer than the
/// whole budget. An empty id is the one exception and it derives to nothing,
/// because there is nothing to name — the engine rejects that id upstream.
///
/// Pure by design, so the gate can drive it with ids this roadmap has never
/// held. A derivation only ever exercised against live data proves nothing about
/// the next chunk anyone writes.
#[must_use]
pub fn derive(id: &str) -> String {
    let words: Vec<&str> = id
        .split(['-', '_', '/', '.', ' '])
        .filter(|w| !w.is_empty())
        .collect();

    if words.is_empty() {
        // No word characters at all. The id is still identity, so clamp it
        // rather than inventing a label that points at nothing.
        return clamp(id.trim());
    }

    // Leading articles go first, but never the last word standing: an id that is
    // literally "the" should name itself rather than name nothing.
    let mut start = 0;
    while start + 1 < words.len()
        && LEADING_NOISE.contains(&words[start].to_ascii_lowercase().as_str())
    {
        start += 1;
    }

    // Take words while both budgets hold. Checked against the *rendered* length
    // including separators, so the result needs no second truncation pass.
    let mut taken: Vec<&str> = Vec::new();
    let mut width = 0usize;
    for w in &words[start..] {
        if taken.len() == MAX_WORDS {
            break;
        }
        let sep = usize::from(!taken.is_empty());
        let next = width + sep + w.chars().count();
        if !taken.is_empty() && next > NAME_BUDGET {
            break;
        }
        taken.push(w);
        width = next;
    }

    // Trailing connectives are trimmed after the cut, not before it: a word is
    // only noise once the word it was reaching for has been dropped. Never trims
    // to nothing, for the same reason the leading pass does not.
    while taken.len() > 1
        && TRAILING_NOISE.contains(&taken[taken.len() - 1].to_ascii_lowercase().as_str())
    {
        taken.pop();
    }

    let rendered: Vec<String> = taken
        .iter()
        .enumerate()
        .map(|(i, w)| case_word(w, i == 0))
        .collect();

    clamp(&rendered.join(" "))
}

/// Uppercase a known acronym, capitalize the leading word, otherwise leave the
/// word lowercase. Sentence case rather than title case — a label is a name, not
/// a headline.
fn case_word(word: &str, leading: bool) -> String {
    let lower = word.to_ascii_lowercase();
    if ACRONYMS.contains(&lower.as_str()) {
        return lower.to_ascii_uppercase();
    }
    if !leading {
        return lower;
    }
    let mut chars = lower.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => lower,
    }
}

/// Cut to [`NAME_BUDGET`] characters. The last line of defence, reached only by
/// a single word longer than the whole budget — character-wise so it cannot
/// split a multi-byte character.
fn clamp(s: &str) -> String {
    s.chars().take(NAME_BUDGET).collect::<String>()
}

/// Separate seeds that collided. [`fn@derive`] keeps an id's HEAD words, so ids
/// that differ only in their tail land on the same label — measured across the
/// 58 real stores on this machine, 770 of 8089 chunks wore a label at least one
/// sibling in the same store also wore, 99% of them inside one region, and every
/// one of them was still the untouched seed. Two marks in one territory wearing
/// one word are two marks a reader cannot tell apart.
///
/// Takes the store's whole `(id, name)` table and returns only the rewrites, so
/// tests drive it with tables production has never held. Only a name still equal
/// to `derive(id)` is ever a candidate: that equality proves no human has
/// touched it, which is what keeps an authored label — and any label that does
/// not collide — out of reach by construction rather than by policy.
///
/// Per colliding class: the common word-prefix of the ids is computed; a member
/// with no words beyond it keeps the label the class tied at (at the top level
/// that is its own seed, so the family head does not move); every other member
/// wears the last shared word as context plus its own distinguishing tail,
/// through the same word-cap and budget machinery as the seed. Members still
/// tied recurse on their strictly-longer shared prefix; a proposal already worn
/// elsewhere in the store widens its context window until free. Deterministic
/// and order-independent (classes and members are processed in sorted order),
/// and idempotent: a rewritten name no longer equals its seed, so the next pass
/// leaves it alone.
#[must_use]
pub fn repair_collisions(table: &[(&str, &str)]) -> Vec<(String, String)> {
    use std::collections::{BTreeMap, BTreeSet};

    let mut by_label: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, (_, name)) in table.iter().enumerate() {
        by_label.entry(name).or_default().push(i);
    }

    // Labels not ours to move: singletons, authored names (mixed classes keep
    // their authored members), and everything a repaired class settles on.
    let mut taken: BTreeSet<String> = BTreeSet::new();
    let mut classes: Vec<(&str, Vec<&str>)> = Vec::new();
    for (label, idxs) in &by_label {
        if label.trim().is_empty() {
            continue; // nameless rows are backfill's job, not a label class
        }
        let (seeds, authored): (Vec<usize>, Vec<usize>) = idxs
            .iter()
            .copied()
            .partition(|&i| table[i].1 == derive(table[i].0));
        if idxs.len() >= 2 && seeds.len() >= 2 {
            classes.push((label, seeds.iter().map(|&i| table[i].0).collect()));
            for i in authored {
                taken.insert(table[i].1.to_string());
            }
        } else {
            for &i in idxs {
                taken.insert(table[i].1.to_string());
            }
        }
    }

    let mut out: Vec<(String, String)> = Vec::new();
    for (tie, ids) in classes {
        let members: Vec<(&str, Vec<&str>)> = ids
            .iter()
            .map(|id| {
                (
                    *id,
                    id.split(['-', '_', '/', '.', ' '])
                        .filter(|w| !w.is_empty())
                        .collect(),
                )
            })
            .collect();
        repair_class(&members, tie, &mut taken, &mut out);
    }

    // Emit only real rewrites, in a stable order.
    let current: BTreeMap<&str, &str> = table.iter().map(|(id, name)| (*id, *name)).collect();
    out.retain(|(id, name)| current.get(id.as_str()).copied() != Some(name.as_str()));
    out.sort();
    out
}

/// Relabel one class of ids whose labels currently tie at `tie`. Members are
/// processed in sorted-id order so the result cannot depend on store order.
fn repair_class(
    members: &[(&str, Vec<&str>)],
    tie: &str,
    taken: &mut std::collections::BTreeSet<String>,
    out: &mut Vec<(String, String)>,
) {
    let mut common = members[0].1.len();
    for (_, words) in members {
        let mut i = 0;
        while i < common && i < words.len() && words[i] == members[0].1[i] {
            i += 1;
        }
        common = common.min(i);
    }

    let mut sorted: Vec<&(&str, Vec<&str>)> = members.iter().collect();
    sorted.sort_by_key(|(id, _)| *id);

    let mut proposals: std::collections::BTreeMap<String, Vec<&(&str, Vec<&str>)>> =
        std::collections::BTreeMap::new();
    for m in sorted {
        let label = if m.1.len() == common {
            // No distinguishing words left: keep the label the class tied at —
            // at the top level, its own seed, so the family head never moves.
            tie.to_string()
        } else {
            // Context + tail, widening the context window while the proposal is
            // already worn outside the class.
            let mut label = String::new();
            for ctx in 1..=common + 1 {
                let from = common.saturating_sub(ctx);
                let mut words: Vec<&str> = m.1[from..common].to_vec();
                let context_len = words.len();
                words.extend_from_slice(&m.1[common..]);
                label = select_from(&words, context_len);
                if !taken.contains(&label) {
                    break;
                }
            }
            label
        };
        proposals.entry(label).or_default().push(m);
    }

    for (label, ms) in proposals {
        if ms.len() == 1 && !taken.contains(&label) {
            taken.insert(label.clone());
            out.push((ms[0].0.to_string(), label));
        } else if ms.len() > 1 && ms.iter().all(|m| m.1.len() > common) {
            // Still tied: these ids share a strictly longer prefix, so the
            // recursion advances and terminates at word exhaustion.
            let subset: Vec<(&str, Vec<&str>)> = ms.iter().map(|m| (m.0, m.1.clone())).collect();
            repair_class(&subset, &label, taken, out);
        } else {
            // Unresolvable within the words the ids carry. Keep the tie rather
            // than invent content — the residual is measured, not hidden.
            for m in ms {
                taken.insert(label.clone());
                out.push((m.0.to_string(), label.clone()));
            }
        }
    }
}

/// [`fn@derive`]'s take-loop over an explicit word list. `must_reach` is the
/// index of the first word carrying distinguishing content; when the budget
/// cannot reach it the context words before it are dropped, because a label
/// that spends the whole budget on shared context has separated nothing.
fn select_from(words: &[&str], must_reach: usize) -> String {
    let mut list = words;
    let mut must = must_reach;
    loop {
        let mut taken: Vec<&str> = Vec::new();
        let mut width = 0usize;
        for w in list {
            if taken.len() == MAX_WORDS {
                break;
            }
            let sep = usize::from(!taken.is_empty());
            let next = width + sep + w.chars().count();
            if !taken.is_empty() && next > NAME_BUDGET {
                break;
            }
            taken.push(w);
            width = next;
        }
        if taken.len() > must || must == 0 || list.len() == 1 {
            while taken.len() > 1
                && TRAILING_NOISE.contains(&taken[taken.len() - 1].to_ascii_lowercase().as_str())
            {
                taken.pop();
            }
            let rendered: Vec<String> = taken
                .iter()
                .enumerate()
                .map(|(i, w)| case_word(w, i == 0))
                .collect();
            return clamp(&rendered.join(" "));
        }
        list = &list[must..];
        must = 0;
    }
}

#[cfg(test)]
#[path = "name.tests.rs"]
mod tests;
