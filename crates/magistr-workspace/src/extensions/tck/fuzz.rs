/// Fuzz-corpus tuning for the lang-pack chapter; the defaults are
/// deterministic and CI-sized. The corpus is byte-identical run to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TckFuzz {
    /// Seeded mutations on top of the fixed edge-case seeds.
    pub iterations: usize,
    /// xorshift32 seed — any fixed value; stability is what matters.
    pub seed: u32,
}

impl Default for TckFuzz {
    fn default() -> Self {
        Self {
            iterations: 64,
            seed: 0x5443_4b31, // "TCK1"
        }
    }
}

/// Fixed edge-case seeds every corpus starts from.
fn seed_cases() -> Vec<String> {
    vec![
        String::new(),
        "src/main.ts".into(),
        "fn main() { println!(\"hello\"); }".into(),
        "päth/withõ/ünicode.gleam".into(),
        "null\u{0}byte.txt".into(),
        "a".repeat(4096),
        format!("{}deep{}", "[".repeat(256), "]".repeat(256)),
        "\t\r\n\u{1b}[31mansi\u{1b}[0m\u{7}".into(),
        "🦀🧩\u{10FFFF}".into(),
    ]
}

const MUTATION_CHARS: &str = " abcdefghijklmnopqrstuvwxyz/._-{}()[]\"'\\\u{0}é世";

impl TckFuzz {
    /// The seeds plus `iterations` seeded mutations, in order.
    #[must_use]
    pub fn corpus(&self) -> Vec<String> {
        let seeds = seed_cases();
        let mut rand = xorshift32(self.seed);
        let mut corpus = seeds.clone();
        for _ in 0..self.iterations {
            let base = &seeds[(rand() as usize) % seeds.len()];
            corpus.push(mutate(base, &mut rand));
        }
        corpus
    }
}

fn xorshift32(seed: u32) -> impl FnMut() -> u32 {
    let mut state = if seed == 0 { 1 } else { seed };
    move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    }
}

/// One seeded mutation of one base case: truncate, splice, repeat or reverse.
/// Mutation works on chars — Rust strings cannot hold the lone surrogates the
/// retained JS corpus produces by slicing UTF-16 units — so the native corpus
/// is deterministic but not byte-identical to the reference's.
fn mutate(base: &str, rand: &mut impl FnMut() -> u32) -> String {
    let chars: Vec<char> = base.chars().collect();
    let letters: Vec<char> = MUTATION_CHARS.chars().collect();
    match rand() % 4 {
        0 => chars[..(rand() as usize) % (chars.len() + 1)]
            .iter()
            .collect(),
        1 => {
            let at = (rand() as usize) % (chars.len() + 1);
            let letter = letters[(rand() as usize) % letters.len()];
            let count = (rand() as usize) % 16 + 1;
            let mut out: String = chars[..at].iter().collect();
            out.extend(std::iter::repeat_n(letter, count));
            out.extend(&chars[at..]);
            out
        }
        2 => {
            let mut out = base.to_owned();
            out.extend(&chars[..(rand() as usize) % (chars.len() + 1)]);
            out
        }
        _ => chars.iter().rev().collect(),
    }
}
