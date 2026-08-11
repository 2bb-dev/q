use super::*;

#[test]
fn latin_query_finds_cyrillic_text() {
    assert!(matches("улучшить конфиги", "uluchshit"));
    assert!(matches("улучшить конфиги", "konfigi"));
}

#[test]
fn both_common_transliteration_conventions_match() {
    // `я` is written `ia` by one convention and `ya` by another.
    assert!(matches("работа прометея", "prometeya"));
    assert!(matches("работа прометея", "prometeia"));
    assert!(matches("локальный дефект", "lokalnyy"));
    assert!(matches("локальный дефект", "lokalnyi"));
}

#[test]
fn cyrillic_query_still_finds_cyrillic_text() {
    assert!(matches("улучшить конфиги", "улучш"));
    assert!(matches("Дополнительно найден дефект", "ДЕФЕКТ"));
}

#[test]
fn composed_and_decomposed_forms_match_each_other() {
    let composed = "\u{043D}\u{0430}\u{0439}\u{0442}\u{0438}";
    let decomposed = "\u{043D}\u{0430}\u{0438}\u{0306}\u{0442}\u{0438}";
    assert_eq!(fold(composed), fold(decomposed));
    assert!(matches(composed, decomposed));
    assert!(matches(decomposed, composed));
}

#[test]
fn accents_are_ignored_in_both_directions() {
    assert!(matches("café Müller", "cafe muller"));
    assert!(matches("cafe muller", "café"));
}

#[test]
fn plain_ascii_search_is_unchanged() {
    assert!(matches("add memory, cron implementation", "cron"));
    assert!(!matches("add memory, cron implementation", "docker"));
}

#[test]
fn empty_query_matches_everything() {
    assert!(matches("anything", ""));
    assert!(matches("", "   "));
    assert!(Query::new("  ").is_empty());
    assert!(!Query::new("x").is_empty());
}

#[test]
fn prefolded_matching_agrees_with_one_shot_matching() {
    let texts = ["улучшить конфиги", "café Müller", "add cron memory"];
    for needle in ["uluchshit", "конфиги", "cafe", "cron", "docker"] {
        let query = Query::new(needle);
        for text in texts {
            assert_eq!(
                query.is_match_folded(&folded(text)),
                matches(text, needle),
                "text {text:?} needle {needle:?}"
            );
        }
    }
}

#[test]
fn fold_lowercases_and_transliterates() {
    assert_eq!(fold("Дефект"), "defekt");
    assert_eq!(fold("İstanbul"), "istanbul");
}

#[test]
fn soft_sign_apostrophes_do_not_block_matches() {
    assert_eq!(fold("локальный"), "lokalnyi");
    assert!(matches("улучшить конфиги", "uluchshit"));
    assert!(matches("Дополнительно найден", "dopolnitelno"));
}
