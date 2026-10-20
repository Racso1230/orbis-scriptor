//! Seeded fantasy names for settlements and regions.
//!
//! Each world picks two or three "cultures" from a small table. A culture
//! is a syllable grammar: word starts (onsets), vowels, word ends (codas)
//! and a few suffixes. Names are built from random syllables, so the same
//! seed always gives the same names, and no name repeats within a world.

use std::collections::HashSet;

use rand::RngExt;
use rand_chacha::ChaCha8Rng;

/// One culture's sound palette. `&'static [&'static str]` is a slice of
/// string literals baked into the binary (they live for the whole program,
/// hence `'static`).
struct Grammar {
    onsets: &'static [&'static str],
    vowels: &'static [&'static str],
    codas: &'static [&'static str],
    /// Appended to some settlement names.
    suffixes: &'static [&'static str],
    /// Appended to some port names.
    port_suffixes: &'static [&'static str],
    /// Appended to some large-region names.
    region_suffixes: &'static [&'static str],
}

const NORDIC: Grammar = Grammar {
    onsets: &[
        "b", "d", "f", "g", "h", "k", "l", "m", "n", "r", "s", "t", "v", "bj", "dr", "fj", "gr",
        "hr", "sk", "sn", "st", "sv", "th", "tr",
    ],
    vowels: &["a", "e", "i", "o", "u", "y", "ae", "ei", "au"],
    codas: &[
        "", "", "n", "r", "l", "k", "rd", "nd", "lf", "rn", "st", "g",
    ],
    suffixes: &["heim", "vik", "stad", "by", "holm", "dal", "borg", "burg"],
    port_suffixes: &["vik", "havn", "sund"],
    region_suffixes: &["land", "mark", "heim"],
};

const LATINATE: Grammar = Grammar {
    onsets: &[
        "c", "v", "l", "m", "n", "p", "r", "s", "t", "qu", "br", "cr", "pr", "tr", "fl", "d", "g",
    ],
    vowels: &["a", "e", "i", "o", "u", "ia", "io", "ae"],
    codas: &["", "", "", "n", "s", "r", "l", "x"],
    suffixes: &["ium", "ona", "ara", "enza", "ica", "ano", "ora"],
    port_suffixes: &["port", "mara", "ostia"],
    region_suffixes: &["ia", "ania", "ica"],
};

const CELTIC: Grammar = Grammar {
    onsets: &[
        "b", "c", "d", "g", "l", "m", "n", "r", "t", "br", "dr", "gl", "gw", "ll", "rh", "bl", "cl",
    ],
    vowels: &["a", "e", "i", "o", "u", "ae", "ai", "ea", "ou", "y"],
    codas: &["", "n", "r", "l", "dd", "th", "ch", "s", "rn", "nn"],
    suffixes: &["more", "wen", "dun", "glen", "ross", "bryn", "lyn", "ford"],
    port_suffixes: &["mouth", "haven", "port"],
    region_suffixes: &["ain", "wyn", "mor"],
};

const DESERT: Grammar = Grammar {
    onsets: &[
        "b", "d", "f", "h", "j", "k", "m", "n", "q", "r", "s", "t", "z", "kh", "sh", "dh",
    ],
    vowels: &["a", "a", "i", "u", "aa", "ai", "ou"],
    codas: &["", "", "r", "n", "m", "d", "b", "sh", "l"],
    suffixes: &["abad", "ar", "esh", "im", "un", "ah"],
    port_suffixes: &["mina", "ahr"],
    region_suffixes: &["an", "ira", "esh"],
};

const EASTERN: Grammar = Grammar {
    onsets: &[
        "h", "k", "m", "n", "r", "s", "t", "y", "ch", "sh", "ts", "l", "w", "j",
    ],
    vowels: &["a", "e", "i", "o", "u", "ai", "ei", "ao"],
    codas: &["", "", "", "n", "ng"],
    suffixes: &["shan", "ji", "ra", "kai", "to", "wan"],
    port_suffixes: &["wan", "ko"],
    region_suffixes: &["ra", "shu", "dao"],
};

const CULTURES: [&Grammar; 5] = [&NORDIC, &LATINATE, &CELTIC, &DESERT, &EASTERN];

/// How a region (a land mass) is named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionSize {
    /// A continent or a large island.
    Large,
    /// A smaller island.
    Island,
}

/// A region name plus how a sentence refers to it ("on the Isle of X",
/// "in Valdmark").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionLabel {
    pub name: String,
    pub phrase: String,
}

/// Hands out unique names for one world.
pub struct NameGenerator {
    rng: ChaCha8Rng,
    /// Indices into `CULTURES` used by this world.
    cultures: Vec<usize>,
    /// Every name given so far, lower-cased.
    used: HashSet<String>,
}

impl NameGenerator {
    /// Picks two or three cultures for the world from `rng`.
    pub fn new(mut rng: ChaCha8Rng) -> NameGenerator {
        let mut order: Vec<usize> = (0..CULTURES.len()).collect();
        // Fisher–Yates shuffle: swap each slot with a random earlier one.
        for i in (1..order.len()).rev() {
            let j = rng.random_range(0..=i);
            order.swap(i, j);
        }
        let count = rng.random_range(2..=3);
        order.truncate(count);
        NameGenerator {
            rng,
            cultures: order,
            used: HashSet::new(),
        }
    }

    /// Number of cultures in this world (settlements pick one by index).
    pub fn culture_count(&self) -> usize {
        self.cultures.len()
    }

    fn grammar(&self, culture: usize) -> &'static Grammar {
        CULTURES[self.cultures[culture % self.cultures.len()]]
    }

    fn pick(&mut self, items: &'static [&'static str]) -> &'static str {
        items[self.rng.random_range(0..items.len())]
    }

    /// A random word of `syllables` syllables, lower case.
    fn word(&mut self, g: &'static Grammar, syllables: usize) -> String {
        let mut out = String::new();
        for s in 0..syllables {
            let last = s + 1 == syllables;
            if s > 0 || self.rng.random_bool(0.85) {
                out.push_str(self.pick(g.onsets));
            }
            out.push_str(self.pick(g.vowels));
            let coda_chance = if last { 0.6 } else { 0.2 };
            if self.rng.random_bool(coda_chance) {
                out.push_str(self.pick(g.codas));
            }
        }
        out
    }

    /// Keeps a candidate if it reads well and is new to this world.
    fn accept(&mut self, candidate: String) -> Option<String> {
        let clean = tidy(&candidate);
        let letters = clean.chars().filter(|c| c.is_alphabetic()).count();
        if !(3..=12).contains(&letters) {
            return None;
        }
        let key = clean.to_lowercase();
        // `insert` returns false when the set already had the value.
        if self.used.insert(key) {
            Some(clean)
        } else {
            None
        }
    }

    /// A settlement name in the given culture. Ports favour sea suffixes
    /// such as "-haven" or "-vik".
    pub fn settlement(&mut self, culture: usize, port: bool) -> String {
        let g = self.grammar(culture);
        for _ in 0..200 {
            // Roll the syllable count first: `self.word(g, self.rng...)` would
            // borrow `self` mutably twice at once (once for the method call,
            // once for the argument), which the borrow checker rejects.
            let short = self.rng.random_range(1..=2);
            let candidate = if port && self.rng.random_bool(0.5) {
                let root = self.word(g, short);
                format!("{root}{}", self.pick(g.port_suffixes))
            } else if self.rng.random_bool(0.35) {
                let root = self.word(g, short);
                format!("{root}{}", self.pick(g.suffixes))
            } else {
                let syllables = if self.rng.random_bool(0.7) { 2 } else { 3 };
                self.word(g, syllables)
            };
            if let Some(name) = self.accept(capitalise(&candidate)) {
                return name;
            }
        }
        self.fallback(g)
    }

    /// A region name, e.g. "Valdmark", "The Corin Reach" or "Isle of Bryn".
    pub fn region(&mut self, culture: usize, size: RegionSize) -> RegionLabel {
        let g = self.grammar(culture);
        for _ in 0..200 {
            let root = capitalise(&self.word(g, 2));
            let form = self.rng.random_range(0..3);
            let (name, phrase) = match (size, form) {
                (RegionSize::Large, 0) => {
                    let n = capitalise(&format!("{}{}", root, self.pick(g.region_suffixes)));
                    (n.clone(), format!("in {n}"))
                }
                (RegionSize::Large, 1) => {
                    let n = format!("The {root} Reach");
                    (n, format!("in the {root} Reach"))
                }
                (RegionSize::Large, _) => (root.clone(), format!("in {root}")),
                (RegionSize::Island, 0 | 1) => {
                    let n = format!("Isle of {root}");
                    (n, format!("on the Isle of {root}"))
                }
                (RegionSize::Island, _) => {
                    let n = format!("{root} Isle");
                    (n.clone(), format!("on {n}"))
                }
            };
            if self.accept(name.clone()).is_some() {
                return RegionLabel { name, phrase };
            }
        }
        let name = self.fallback(g);
        RegionLabel {
            phrase: format!("in {name}"),
            name,
        }
    }

    /// Swaps in a new random stream but keeps the cultures and every name
    /// given so far. The history stage uses its own stream, so adding names
    /// there can never change the settlement names.
    pub fn reseed(&mut self, rng: ChaCha8Rng) {
        self.rng = rng;
    }

    /// A unique root word for a family, e.g. "Varn" (the caller adds
    /// "House"). Short: one or two syllables.
    pub fn family(&mut self, culture: usize) -> String {
        let g = self.grammar(culture);
        for _ in 0..200 {
            let syllables = self.rng.random_range(1..=2);
            let candidate = capitalise(&self.word(g, syllables));
            if let Some(name) = self.accept(candidate) {
                return name;
            }
        }
        self.fallback(g)
    }

    /// A unique given name, e.g. "Isolde".
    pub fn given(&mut self, culture: usize) -> String {
        let g = self.grammar(culture);
        for _ in 0..200 {
            let syllables = if self.rng.random_bool(0.85) { 2 } else { 3 };
            let candidate = capitalise(&self.word(g, syllables));
            if let Some(name) = self.accept(candidate) {
                return name;
            }
        }
        self.fallback(g)
    }

    /// A kingdom name, e.g. "Kingdom of Valdmark", "The Corin Realm" or
    /// "Bryn Dominion".
    pub fn kingdom(&mut self, culture: usize) -> String {
        let g = self.grammar(culture);
        for _ in 0..200 {
            let mut root = capitalise(&self.word(g, 2));
            if self.rng.random_bool(0.4) {
                root = capitalise(&format!("{root}{}", self.pick(g.region_suffixes)));
            }
            let form = self.rng.random_range(0..5);
            // Only the root has to be new; `accept` also checks its length.
            if let Some(root) = self.accept(root) {
                return match form {
                    0 | 1 => format!("Kingdom of {root}"),
                    2 => format!("The {root} Realm"),
                    3 => format!("{root} Dominion"),
                    _ => format!("Principality of {root}"),
                };
            }
        }
        format!("Kingdom of {}", self.fallback(g))
    }

    /// Practically never reached: a long word with a counter is unique.
    fn fallback(&mut self, g: &'static Grammar) -> String {
        let mut n = 2;
        loop {
            let candidate = format!("{}{}", capitalise(&self.word(g, 3)), roman(n));
            let key = candidate.to_lowercase();
            if self.used.insert(key) {
                return candidate;
            }
            n += 1;
        }
    }
}

/// "valdheim" → "Valdheim".
fn capitalise(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Drops a letter that would be the third identical one in a row
/// ("Haaal" → "Haal").
fn tidy(name: &str) -> String {
    let mut out: Vec<char> = Vec::with_capacity(name.len());
    for c in name.chars() {
        let n = out.len();
        let lower = c.to_ascii_lowercase();
        if n >= 2
            && out[n - 1].to_ascii_lowercase() == lower
            && out[n - 2].to_ascii_lowercase() == lower
        {
            continue;
        }
        out.push(c);
    }
    out.into_iter().collect()
}

/// " II", " III", ... for the fallback names.
fn roman(n: u32) -> String {
    const NUMERALS: [&str; 9] = ["II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
    match NUMERALS.get((n as usize).saturating_sub(2)) {
        Some(r) => format!(" {r}"),
        None => format!(" {n}"),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::services::generator::rng::{Stage, stage_rng};

    fn names(seed: u32) -> Vec<String> {
        let mut g = NameGenerator::new(stage_rng(seed, Stage::Names));
        let mut out = Vec::new();
        for i in 0..80 {
            out.push(g.settlement(i % g.culture_count(), i % 3 == 0));
        }
        out.push(g.region(0, RegionSize::Large).name);
        out.push(g.region(1, RegionSize::Island).name);
        out
    }

    #[test]
    fn names_are_unique_and_stable_per_seed() {
        let a = names(42);
        assert_eq!(a, names(42));
        assert_ne!(a, names(43));
        let set: HashSet<String> = a.iter().map(|n| n.to_lowercase()).collect();
        assert_eq!(set.len(), a.len());
        for n in &a {
            assert!(n.chars().next().unwrap().is_uppercase(), "{n}");
        }
    }

    #[test]
    fn worlds_use_two_or_three_cultures() {
        for seed in 0..20 {
            let g = NameGenerator::new(stage_rng(seed, Stage::Names));
            assert!((2..=3).contains(&g.culture_count()));
        }
    }

    #[test]
    fn region_phrases_match_their_names() {
        let mut g = NameGenerator::new(stage_rng(1, Stage::Names));
        for _ in 0..20 {
            let island = g.region(0, RegionSize::Island);
            assert!(island.phrase.starts_with("on "), "{island:?}");
            assert!(island.name.contains("Isle"));
            let large = g.region(1, RegionSize::Large);
            assert!(large.phrase.starts_with("in "), "{large:?}");
        }
    }

    #[test]
    fn tidy_and_capitalise() {
        assert_eq!(tidy("Haaal"), "Haal");
        assert_eq!(capitalise("valdheim"), "Valdheim");
        assert_eq!(roman(3), " III");
    }
}
