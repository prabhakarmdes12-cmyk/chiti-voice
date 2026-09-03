//! Text normalization for Indian English
//!
//! Expands abbreviations, currency amounts, dates, and numbers for natural speech.

use crate::error::VoiceResult;
use std::collections::HashMap;

/// Text normalizer that expands various abbreviations and numbers
pub struct TextNormalizer;

impl TextNormalizer {
    pub fn new() -> Self {
        Self
    }

    /// Normalize text for synthesis
    pub fn normalize(&self, text: &str) -> VoiceResult<String> {
        // TODO: Implement full text normalization:
        // 1. Currency expansion (₹1,25,000 -> "one lakh twenty-five thousand rupees")
        // 2. Date expansion (14/08/1947 -> "fourteenth August nineteen forty-seven")
        // 3. Phone number expansion (98765-43210 -> grouped digits)
        // 4. Abbreviation resolution (PM, CM, IAS, ISRO)
        // 5. Number expansion
        // 6. Unicode normalization
        // 7. Sentence segmentation
        Ok(text.to_string())
    }
}

/// One piece of a synthesis plan: either text that must go through graph2phoneme, or an
/// already-known phoneme string that must bypass it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Ordinary text; the engine phonemises this.
    Text(String),
    /// IPA to use verbatim for the matched word (the pack declared it).
    Phonemes(String),
}

impl Segment {
    #[must_use]
    pub fn is_override(&self) -> bool {
        matches!(self, Self::Phonemes(_))
    }
}

/// Split `text` into segments according to a pack's `pronunciation_overrides` table.
///
/// This is the whole reason the override slot exists: graph2phoneme guessed the product's own name
/// as `tʃˈaːɾi` (docs/research/KOKORO_OFFLINE_SPIKE.md), and no amount of engine work fixes a
/// dictionary that has never heard the word. Doing the substitution *after* phonemisation cannot
/// work — by then the word is already wrong — so the plan has to be decided up front, per token.
///
/// Rules, all locked by tests: the key is the alphanumeric core of a whitespace-delimited token, so
/// `Chiti,` hits while the comma stays text; matching is case-insensitive; and it is whole-token only,
/// so `chitizone` is never rewritten and `Chiti's` does **not** hit (its core is `chitis`) — a pack
/// that wants a possessive form must list it. Those limits are deliberate: rewriting a token the user
/// did not name is how a pronunciation table becomes a spelling corrector.
#[must_use]
pub fn split_for_overrides(text: &str, overrides: &HashMap<String, String>) -> Vec<Segment> {
    if overrides.is_empty() || text.is_empty() {
        return vec![Segment::Text(text.to_owned())];
    }

    let mut out: Vec<Segment> = Vec::new();
    let mut pending = String::new();
    let mut word = String::new();

    let flush_word = |pending: &mut String, word: &mut String, out: &mut Vec<Segment>| {
        if word.is_empty() {
            return;
        }
        // Script-aware on purpose: `is_ascii_alphanumeric` here would strip Devanagari letters from
        // the core, so `\u{091A}chiti\u{0902}` would look like the bare token `chiti` and get rewritten. In a
        // product whose input is Hindi and Tamil, an ASCII-only notion of "word character" is a bug,
        // and it must disagree with the punctuation rule below in exactly zero places.
        let key: String = word.chars().filter(|c| c.is_alphanumeric()).collect();
        match overrides.get(&key.to_ascii_lowercase()).or_else(|| {
            overrides
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(&key))
                .map(|(_, v)| v)
        }) {
            Some(ipa) => {
                // Leading/trailing punctuation stays text: a `"` before a name is not part of it.
                let lead: String = word.chars().take_while(|c| !c.is_alphanumeric()).collect();
                let trail: String = word
                    .chars()
                    .rev()
                    .take_while(|c| !c.is_alphanumeric())
                    .collect::<String>();
                let trail = trail.chars().rev().collect::<String>();
                if !lead.is_empty() {
                    pending.push_str(&lead);
                }
                if !pending.is_empty() {
                    out.push(Segment::Text(std::mem::take(pending)));
                }
                out.push(Segment::Phonemes(ipa.clone()));
                if !trail.is_empty() {
                    pending.push_str(&trail);
                }
            }
            None => pending.push_str(word),
        }
        word.clear();
    };

    for ch in text.chars() {
        if ch.is_whitespace() {
            flush_word(&mut pending, &mut word, &mut out);
            pending.push(ch);
        } else {
            word.push(ch);
        }
    }
    flush_word(&mut pending, &mut word, &mut out);

    if !pending.is_empty() {
        out.push(Segment::Text(pending));
    }
    out
}

impl Default for TextNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Test helper: the segments a caller would feed to two different paths, joined for assertion
/// purposes only. Deliberately not public, because there is no correct way to put IPA and text back
/// into one string for a phonemiser that expects text.
#[cfg(test)]
fn render(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|s| match s {
            Segment::Text(t) => t.clone(),
            Segment::Phonemes(p) => p.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const IPA: &str = "\u{02C8}t\u{0283}\u{026A}ti";

    fn table() -> HashMap<String, String> {
        HashMap::from([("chiti".to_string(), IPA.to_string())])
    }

    #[test]
    fn test_normalizer_creation() {
        let _normalizer = TextNormalizer::new();
    }

    #[test]
    fn normalize_is_still_a_pass_through_and_says_so() {
        // Honest pin: number/date/currency expansion is NOT implemented (the TODO above the body is
        // the claim). If someone implements item 1, this test should fail rather than mislead.
        let n = TextNormalizer::new();
        assert_eq!(n.normalize("\u{20B9}1,25,000").unwrap(), "\u{20B9}1,25,000");
    }

    #[test]
    fn empty_table_or_empty_text_is_one_text_segment() {
        assert_eq!(
            split_for_overrides("hello world", &HashMap::new()),
            vec![Segment::Text("hello world".to_string())]
        );
        assert_eq!(
            split_for_overrides("", &table()),
            vec![Segment::Text(String::new())]
        );
    }

    #[test]
    fn a_matched_word_becomes_its_own_segment() {
        let got = split_for_overrides("Call Chiti now", &table());
        assert_eq!(
            got,
            vec![
                Segment::Text("Call ".to_string()),
                Segment::Phonemes(IPA.to_string()),
                Segment::Text(" now".to_string()),
            ]
        );
        assert!(got.iter().any(Segment::is_override));
        assert_eq!(render(&got), "Call ˈtʃɪti now");
    }

    #[test]
    fn punctuation_stays_text_and_case_is_ignored() {
        let got = split_for_overrides("say \"CHITI,\" loudly", &table());
        assert_eq!(got.len(), 3, "text | phonemes | text: {got:?}");
        assert!(got[1].is_override());
        let joined = render(&got);
        assert!(joined.starts_with("say \""), "leading quote kept as text: {joined:?}");
        assert!(joined.contains(",\""), "trailing punctuation kept as text: {joined:?}");
    }

    #[test]
    fn substring_lookalikes_and_possessives_are_left_alone() {
        let got = split_for_overrides("the chitizone and chiti-voice", &table());
        assert_eq!(
            got.iter().filter(|s| s.is_override()).count(),
            0,
            "no whole-token match may fire here: {got:?}"
        );
        let possessive = split_for_overrides("Chiti's board", &table());
        assert!(
            possessive.iter().all(|s| !s.is_override()),
            "the documented limitation: a possessive core is `chitis`, so it must be listed explicitly: {possessive:?}"
        );
    }

    #[test]
    fn repeated_hits_are_all_applied() {
        let got = split_for_overrides("chiti and chiti", &table());
        assert_eq!(got.iter().filter(|s| s.is_override()).count(), 2);
        assert_eq!(got.len(), 3, "both hits, one text run between them: {got:?}");
    }

    #[test]
    fn non_ascii_word_characters_count_as_word_characters() {
        // `chiti` wrapped in Devanagari letters is one token whose core is NOT `chiti`, so nothing is
        // rewritten. This is the regression test for the ASCII-only key filter that used to be here.
        let got = split_for_overrides("\u{091A}chiti\u{0902}", &table());
        assert!(
            got.iter().all(|s| !s.is_override()),
            "Indic letters are word characters, not punctuation: {got:?}"
        );
        assert_eq!(render(&got), "\u{091A}chiti\u{0902}");

        // While real punctuation still hits, because it is not alphanumeric in any script.
        let punctuated = split_for_overrides("(chiti)", &table());
        assert_eq!(
            punctuated.iter().filter(|s| s.is_override()).count(),
            1,
            "parentheses must not block a match: {punctuated:?}"
        );
    }
}
