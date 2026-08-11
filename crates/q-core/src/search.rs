//! Language-agnostic text matching for prompt search.
//!
//! Both sides of a comparison are transliterated to lowercase ASCII, so a
//! query matches regardless of the script it is typed in, of accents, and of
//! whether the text is stored in composed (NFC) or decomposed (NFD) form.
//!
//! Transliteration is ambiguous: Cyrillic `я` is written `ia` by one common
//! convention and `ya` by another, and people type both. Each side is folded
//! with both schemes and the query matches if any pair agrees.
//!
//! Apostrophes are dropped because the Cyrillic soft and hard signs become
//! `'` (`локальный` -> `lokal'nyi`), which nobody types when searching.
//!
//! Folding allocates, so callers that match one query against many texts
//! should fold the query once into a [`Query`] and cache the folded texts as
//! [`Folded`] values.

/// Transliterates to lowercase ASCII for comparison.
pub fn fold(text: &str) -> String {
    strip_apostrophes(&deunicode::deunicode(text).to_lowercase())
}

fn strip_apostrophes(text: &str) -> String {
    text.replace(['\'', '’'], "")
}

/// One text folded under both transliteration conventions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folded([String; 2]);

/// Folds `text` once so it can be matched against many queries.
pub fn folded(text: &str) -> Folded {
    Folded([
        fold(text),
        strip_apostrophes(&any_ascii::any_ascii(text).to_lowercase()),
    ])
}

/// A search term folded once, ready to match against many texts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    needles: Folded,
    empty: bool,
}

impl Query {
    pub fn new(needle: &str) -> Self {
        let needles = folded(needle);
        let empty = needles.0.iter().all(|needle| needle.trim().is_empty());
        Self { needles, empty }
    }

    /// An empty query matches everything.
    pub fn is_empty(&self) -> bool {
        self.empty
    }

    /// Whether an already folded text matches this query.
    pub fn is_match_folded(&self, folded: &Folded) -> bool {
        self.empty
            || self
                .needles
                .0
                .iter()
                .any(|needle| folded.0.iter().any(|hay| hay.contains(needle.as_str())))
    }

    /// Whether `text` matches this query, folding it on the spot.
    pub fn is_match(&self, text: &str) -> bool {
        self.empty || self.is_match_folded(&folded(text))
    }
}

/// Whether `haystack` contains `needle`, ignoring script, case, and accents.
///
/// Folds both sides on every call; prefer [`Query`] when matching repeatedly.
pub fn matches(haystack: &str, needle: &str) -> bool {
    Query::new(needle).is_match(haystack)
}

#[cfg(test)]
#[path = "../tests/unit/search.rs"]
mod tests;
