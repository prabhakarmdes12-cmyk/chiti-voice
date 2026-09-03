//! Splitting one caller's phonemes into graph-sized utterances, deterministically.
//!
//! This is a separate concern rather than a loop inside an engine for two reasons, and both are the
//! kind that get missed:
//!
//! * **Chunking is a prosody decision.** The reference reads the style row at `n_tokens`, so the row a
//!   voice uses is chosen by how many tokens the utterance has -- which means the same sentence sounds
//!   different depending on where it was split. `docs/ROADMAP_EMBEDDED.md` lists the absent policy as
//!   the more dangerous of the two undefined engine behaviours; [`plan_pieces`] is the policy, and an
//!   engine is expected to call it instead of re-deriving boundaries that it cannot describe to a user.
//! * **`encode` truncates, silently.** [`crate::phoneme_tokens::encode`] cuts at the 512-token window,
//!   so a long utterance loses the end of what the caller asked to hear, with nothing to show for it.
//!   Planning first makes the budget impossible to overflow: [`DEFAULT_MAX_UNITS`] leaves truncation
//!   unreachable, and `truncation_cannot_fire_on_a_planned_utterance` asserts that directly against
//!   `encode` rather than trusting the arithmetic here.
//!
//! # What a unit is
//!
//! One character of the phoneme string *after* the vocabulary filter, plus one for each space between
//! runs -- because that is exactly what lands between the two `$` wraps. Budgeting in units rather than
//! in the caller's text length is the only honest choice: G2P changes the count, and the graph window
//! bounds tokens, not letters.
//!
//! # Ordering contract with persona overrides
//!
//! Pieces are phonemes, not text. `split_for_overrides` runs before G2P (it decides which runs bypass
//! the phonemiser), G2P runs on the text runs, and planning runs on the result. A boundary is never
//! placed inside a [`Piece`], which is what stops a single-word override -- the one that fixes the
//! product's own name -- from being cut in half so that its stressed vowel ends up in the next
//! utterance with a different style row.

use crate::error::{VoiceError, VoiceErrorCode, VoiceResult};
use crate::phoneme_tokens::{encode, style_row, MAX_PHONEME_UNITS};

/// The largest content-token count a planned utterance may use.
///
/// `MAX_PHONEME_UNITS` (510) content tokens is the physical limit -- 512 slots minus the two `$` wraps
/// -- and this is one below it, because `n_tokens` clamps the row index to `MAX_PHONEME_UNITS - 1`.
/// Budgeting to 509 means the clamp is never what selects a row, so every [`Utterance`] satisfies
/// `style_row == units`: the row a chunk reads is exactly its length.
pub const DEFAULT_MAX_UNITS: usize = MAX_PHONEME_UNITS - 1;

/// Punctuation that closes a chunk. Commas are deliberately absent: a boundary is a decision about
/// prosody, and "the window is full" is the only reason strong enough to take one mid-sentence.
const CHUNK_FINALS: [char; 5] = ['.', '!', '?', ';', ':'];

/// How [`plan_pieces`] draws boundaries.
///
/// Part of a persona's sound, not just an internal detail: two daemons with different policies will
/// read different style rows for one sentence. A pack that wants a specific sound therefore has to say
/// which policy it was tuned under, and `docs/ROADMAP_EMBEDDED.md` keeps that as an open manifest item
/// rather than letting a default become a claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanPolicy {
    /// Hard ceiling on content tokens per utterance, spaces included.
    pub max_units: usize,
    /// A chunk is closed at sentence-final punctuation only once it has this many units, and a chunk
    /// shorter than this is folded into its predecessor when that still fits the ceiling. Both jobs are
    /// the same concern: a two-token utterance is an odd prosody and an odd sounding phrase.
    pub min_chunk_units: usize,
}

impl Default for PlanPolicy {
    fn default() -> Self {
        Self { max_units: DEFAULT_MAX_UNITS, min_chunk_units: 8 }
    }
}

impl PlanPolicy {
    /// Reject a policy that cannot be honoured, before any input is walked.
    pub fn validate(&self) -> VoiceResult<()> {
        if self.max_units == 0 || self.max_units > MAX_PHONEME_UNITS {
            return Err(VoiceError::new(
                VoiceErrorCode::NormalizationFailed,
                format!(
                    "plan policy max_units {} is outside 1..={MAX_PHONEME_UNITS}; \
                     above that the window itself, not the policy, decides where an utterance ends",
                    self.max_units
                ),
            ));
        }
        if self.min_chunk_units > self.max_units {
            return Err(VoiceError::new(
                VoiceErrorCode::NormalizationFailed,
                format!(
                    "plan policy min_chunk_units {} exceeds max_units {}, which would fold every \
                     chunk into nothing",
                    self.min_chunk_units, self.max_units
                ),
            ));
        }
        Ok(())
    }
}

/// One indivisible run of phonemes: a word, or one pronouncer override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// Phonemes as they will be handed to `encode`, `$` wraps excluded.
    pub phonemes: String,
    /// True when this run came from a persona `pronunciation_overrides` entry and so bypassed G2P.
    /// Planning never splits one, and reports which utterances contain one.
    pub is_override: bool,
}

impl Piece {
    /// A run the phonemiser produced.
    #[must_use]
    pub fn phonemes(value: impl Into<String>) -> Self {
        Self { phonemes: value.into(), is_override: false }
    }

    /// A run that came from the pack, not from G2P -- kept whole for its own sake.
    #[must_use]
    pub fn from_override(value: impl Into<String>) -> Self {
        Self { phonemes: value.into(), is_override: true }
    }

    /// Tokens this run occupies: one per character, because `encode` gives an unmapped character a
    /// `PAD` slot rather than dropping it. So the budget is the caller's length, `style_row == units`
    /// holds for any input, and neither number depends on which characters a given table happens to
    /// carry. It used to be the *spellable* count, which is one short per unmapped character -- exactly
    /// the size of a style-row drift that no assertion would have caught.
    #[must_use]
    pub fn units(&self) -> usize {
        self.phonemes.chars().count()
    }
}

/// One utterance: what to feed the graph, and which style row that implies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Utterance {
    /// The phonemes to encode, joined by single spaces, verbatim: an unmapped character is
    /// still in here, and `encode` gives it a `PAD` slot rather than removing it.
    pub phonemes: String,
    /// Content tokens [`Utterance::phonemes`] occupies.
    pub units: usize,
    /// The style row this utterance reads. Equals [`Utterance::units`] for any plan inside the
    /// window, which is the property [`DEFAULT_MAX_UNITS`] exists to guarantee.
    pub style_row: usize,
    /// How many pieces landed in this utterance.
    pub pieces: usize,
    /// True when at least one piece was a pronouncer override.
    pub has_override: bool,
}

/// A planned sequence of utterances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Utterances in playback order.
    pub utterances: Vec<Utterance>,
    /// The policy that produced this plan, echoed for logging and for the CLI's prosody report: an
    /// audio artefact without its policy is not reproducible.
    pub policy: PlanPolicy,
}

impl Plan {
    /// Number of utterances.
    #[must_use]
    pub fn len(&self) -> usize {
        self.utterances.len()
    }

    /// Whether the input produced nothing the graph would receive.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.utterances.is_empty()
    }

    /// The rows in playback order. Two plans of the same sentence that differ here differ in sound.
    #[must_use]
    pub fn style_rows(&self) -> Vec<usize> {
        self.utterances.iter().map(|u| u.style_row).collect()
    }
}

/// Total tokens a run of pieces occupies, spaces between them included.
fn run_units(run: &[&Piece]) -> usize {
    run.iter().map(|piece| piece.units()).sum::<usize>() + run.len().saturating_sub(1)
}

fn ends_chunk(phonemes: &str) -> bool {
    phonemes
        .chars()
        .next_back()
        .is_some_and(|last| CHUNK_FINALS.contains(&last))
}

fn utterance_from(run: &[&Piece]) -> Utterance {
    let phonemes = run
        .iter()
        .map(|piece| piece.phonemes.clone())
        .collect::<Vec<String>>()
        .join(" ");
    let units = phonemes.chars().count();
    let encoded = encode(&phonemes);
    Utterance {
        phonemes,
        units,
        style_row: style_row(&encoded),
        pieces: run.len(),
        has_override: run.iter().any(|piece| piece.is_override),
    }
}

/// Draw the boundaries.
///
/// Rules, in the order they apply: a piece is never split; a chunk closes at sentence-final punctuation
/// once it reaches `min_chunk_units`; a piece that would overflow the ceiling starts a new chunk; and a
/// short final chunk is folded back into its predecessor when that stays inside the ceiling. A single
/// piece larger than the ceiling is an error -- there is no boundary to take inside a word, and
/// truncating it would lose the sound the caller asked for.
pub fn plan_pieces(pieces: &[Piece], policy: &PlanPolicy) -> VoiceResult<Plan> {
    policy.validate()?;

    let mut runs: Vec<Vec<&Piece>> = Vec::new();
    let mut current: Vec<&Piece> = Vec::new();
    let mut current_units = 0usize;

    for piece in pieces {
        let units = piece.units();
        if units == 0 {
            // `encode` would drop every character of it. Skipping here keeps a chunk's `units` equal
            // to what the graph receives, instead of promising a token that never exists.
            continue;
        }
        if units > policy.max_units {
            return Err(VoiceError::new(
                VoiceErrorCode::NormalizationFailed,
                format!(
                    "a single run needs {units} tokens, above the {} this policy allows: no chunk \
                     boundary is available inside it, so it is refused rather than truncated",
                    policy.max_units
                ),
            ));
        }

        let space = usize::from(!current.is_empty());
        if current_units + space + units > policy.max_units {
            runs.push(std::mem::take(&mut current));
            current_units = 0;
        }
        let space = usize::from(!current.is_empty());
        current_units += space + units;
        current.push(piece);

        if current_units >= policy.min_chunk_units && ends_chunk(&piece.phonemes) {
            runs.push(std::mem::take(&mut current));
            current_units = 0;
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }

    // Fold short tails back, oldest-first so a run that grew can absorb the next one too.
    let mut index = 1;
    while index < runs.len() {
        let joined = run_units(&runs[index - 1]) + 1 + run_units(&runs[index]);
        if run_units(&runs[index]) < policy.min_chunk_units && joined <= policy.max_units {
            let tail = runs.remove(index);
            runs[index - 1].extend(tail);
            // No back-tracking: the merged run only grew, and folding is a rule about short runs.
        }
        index += 1;
    }

    let mut utterances = Vec::with_capacity(runs.len());
    for run in &runs {
        utterances.push(utterance_from(run));
    }

    Ok(Plan { utterances, policy: *policy })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phoneme_tokens::{strip_to_vocab, MAX_TOKENS};

    /// ASCII phoneme-ish runs. `units` is the literal character count for all of them -- `encode` no
    /// longer changes a string's length -- which is what makes the numbers below checkable by hand.
    fn words(text: &str) -> Vec<Piece> {
        text.split_whitespace().map(Piece::phonemes).collect()
    }

    #[test]
    fn nothing_in_is_nothing_out_and_something_unspellable_is_still_something() {
        let plan = plan_pieces(&[], &PlanPolicy::default()).unwrap();
        assert!(plan.is_empty(), "no pieces, no utterances");
        assert!(plan.utterances.is_empty(), "and the plan and its Vec agree");

        // The interesting half, and it changed when `encode` stopped dropping: a run of characters the
        // table lacks (U+2603 is certainly absent from a 115-symbol IPA table) is *not* free, because the
        // encoder gives each one a `PAD` slot. So it is planned as an utterance and gets a row, exactly as
        // the reference would -- the sound is wrong, but the length, and therefore the prosody, is honest.
        let unspellable = Piece::phonemes("\u{2603}".to_string());
        assert_eq!(unspellable.units(), 1, "one character, one token, pad or no pad");
        let plan = plan_pieces(&[unspellable], &PlanPolicy::default()).unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan.style_rows(), vec![1], "and a row of its own, as upstream's ids would");
    }

    #[test]
    fn every_character_is_billed_because_the_encoder_pads_rather_than_drops() {
        // U+026B (l-with-tilde, which some Indic G2P output produces) is not in the table, so the graph
        // gets a PAD for it -- and PAD is a token. Budgeting on the spellable characters only used to be
        // one short per such character, and because the style row *is* the length, that is a prosody bug
        // hiding in a counting rule. Escapes, so no transport encoding can mangle the character.
        let piece = Piece::phonemes("ka\u{026B}la".to_string());
        assert_eq!(piece.units(), 5, "five characters, five tokens, unmapped or not");

        let plan = plan_pieces(&[piece], &PlanPolicy::default()).unwrap();
        let utterance = &plan.utterances[0];
        assert_eq!(utterance.units, 5);
        assert_eq!(utterance.style_row, 5, "the row is that length, not the filtered count");
        assert_eq!(encode(&utterance.phonemes).len(), 7, "$ + 5 + $, nothing removed");
        assert_eq!(
            strip_to_vocab(&utterance.phonemes).chars().count(),
            4,
            "the filter is still the other behaviour, and is why it is not used for budgets"
        );
    }

    #[test]
    fn truncation_cannot_fire_on_a_planned_utterance() {
        // Far more input than the window holds. Every planned utterance must fit it exactly, which is
        // the difference between a long paragraph and a long paragraph minus its last clause.
        let sentence = "slovo na proveri, to je dovoljno dugo.";
        let pieces: Vec<Piece> = (0..200).flat_map(|_| words(sentence)).collect();
        let joined: String = pieces
            .iter()
            .map(|p| p.phonemes.clone())
            .collect::<Vec<String>>()
            .join(" ");
        assert!(
            joined.chars().count() > MAX_TOKENS,
            "this test is pointless unless the input overflows the window"
        );

        let plan = plan_pieces(&pieces, &PlanPolicy::default()).unwrap();
        assert!(plan.len() > 1, "one utterance cannot hold it: {:?}", plan.len());
        for utterance in &plan.utterances {
            let encoded = encode(&utterance.phonemes);
            assert_eq!(encoded.len(), utterance.units + 2, "nothing was truncated away");
            assert!(utterance.units <= DEFAULT_MAX_UNITS);
            assert_eq!(utterance.style_row, utterance.units, "no row was selected by the clamp");
        }
    }

    #[test]
    fn planning_loses_nothing_and_invents_nothing() {
        let pieces = words("ha ha ha. ti si tu, a ja? ne.");
        let plan = plan_pieces(&pieces, &PlanPolicy { max_units: 9, min_chunk_units: 2 }).unwrap();
        let planned = plan
            .utterances
            .iter()
            .map(|u| u.phonemes.as_str())
            .collect::<Vec<&str>>()
            .join(" ");
        let expected: String = pieces
            .iter()
            .map(|p| p.phonemes.clone())
            .collect::<Vec<String>>()
            .join(" ");
        assert_eq!(planned, expected, "the chunks must rejoin into the input, spaces included");
        let pieces_total: usize = plan.utterances.iter().map(|u| u.pieces).sum();
        assert_eq!(pieces_total, pieces.len(), "no run may be dropped or split");
    }

    #[test]
    fn an_override_survives_a_boundary_whole() {
        // Sized so the ceiling falls inside the filler, right where the override sits: the rule that
        // matters is that the override is never what gets cut.
        let mut pieces: Vec<Piece> = Vec::new();
        for _ in 0..6 {
            pieces.push(Piece::phonemes("aaaa".to_string()));
        }
        pieces.push(Piece::from_override("t\u{0283}\u{0261}\u{026A}ti".to_string()));
        for _ in 0..6 {
            pieces.push(Piece::phonemes("bbbb".to_string()));
        }

        let plan = plan_pieces(&pieces, &PlanPolicy { max_units: 12, min_chunk_units: 1 }).unwrap();
        let with_override: Vec<&Utterance> =
            plan.utterances.iter().filter(|u| u.has_override).collect();
        assert_eq!(with_override.len(), 1, "the override belongs to exactly one chunk");
        let spelled = "\u{0283}\u{0261}\u{026A}";
        assert!(
            with_override[0].phonemes.contains(spelled),
            "and it must be there complete, not split across two rows: {:?}",
            with_override[0].phonemes
        );
    }

    #[test]
    fn an_unsplittable_run_is_refused_with_its_numbers() {
        let too_big = Piece::phonemes("a".repeat(DEFAULT_MAX_UNITS + 1));
        let err = plan_pieces(&[too_big], &PlanPolicy::default()).unwrap_err();
        assert_eq!(err.code(), VoiceErrorCode::NormalizationFailed);
        let text = err.to_string();
        assert!(text.contains("no chunk boundary"), "{text}");
        assert!(
            text.contains(DEFAULT_MAX_UNITS.to_string().as_str()),
            "the message must quote the ceiling it exceeded: {text}"
        );
    }

    #[test]
    fn a_policy_that_cannot_be_honoured_fails_before_any_input_is_walked() {
        for bad in [PlanPolicy { max_units: 0, min_chunk_units: 1 }, PlanPolicy {
            max_units: MAX_PHONEME_UNITS + 1,
            min_chunk_units: 1,
        }] {
            let err = plan_pieces(&words("ha."), &bad).unwrap_err();
            assert_eq!(err.code(), VoiceErrorCode::NormalizationFailed);
            assert!(err.to_string().contains("max_units"), "{err}");
        }
        let inverted = PlanPolicy { max_units: 8, min_chunk_units: 9 };
        assert!(plan_pieces(&words("ha."), &inverted)
            .unwrap_err()
            .to_string()
            .contains("min_chunk_units"));
    }

    #[test]
    fn sentence_punctuation_closes_a_chunk_once_the_minimum_is_met() {
        let policy = PlanPolicy { max_units: 12, min_chunk_units: 4 };
        let plan = plan_pieces(&words("ha. ha. haha."), &policy).unwrap();
        assert_eq!(plan.len(), 2, "two sentences fill a chunk, the third starts the next");
        assert_eq!(plan.style_rows(), vec![7, 5], "rows are lengths, so they must add up by hand");
        assert_eq!(plan.policy, policy, "the policy travels with the plan it produced");

        // Below the minimum, punctuation does not fire: 13 units is a whole chunk under `max_units`,
        // and the sentences stay together. That is the rule that keeps a lone interjection from
        // arriving as its own prosody -- and the reason a short tail needs the fold below.
        let patient = PlanPolicy { max_units: 20, min_chunk_units: 9 };
        let together = plan_pieces(&words("ha. ha. haha."), &patient).unwrap();
        assert_eq!(together.len(), 1, "nothing is long enough to break for");
        assert_eq!(together.style_rows(), vec![13]);
    }

    #[test]
    fn the_same_sentence_under_two_policies_is_two_different_sounds()
    {
        let pieces = words("ovo je jedna duga recenica koja hoce da se slusa.");
        let one = plan_pieces(&pieces, &PlanPolicy::default()).unwrap();
        let many = plan_pieces(
            &pieces,
            &PlanPolicy { max_units: 12, min_chunk_units: 4 },
        )
        .unwrap();

        assert_eq!(one.len(), 1, "the default ceiling holds a sentence of this size");
        assert!(many.len() > 1, "a tighter one splits it");
        assert_ne!(one.style_rows(), many.style_rows());
        // Asserted as a relationship, not a constant, so the test survives a regenerated table -- and
        // it is a real relationship: "duga" contains an ASCII g, which the 115-entry table does not
        // have. These two totals were one apart until `encode` learned to pad; they must not part again.
        let typed: usize =
            pieces.iter().map(|p| p.phonemes.chars().count()).sum::<usize>() + pieces.len() - 1;
        assert_eq!(
            one.style_rows()[0],
            typed,
            "one chunk reads the row for the whole sentence: the row is the caller's length, and since \
             `encode` pads rather than drops, that is also the graph's length -- \"duga\" holds an ASCII \
             g the table lacks, and used to make these two numbers differ by one"
        );
    }

    #[test]
    fn a_short_tail_folds_back_only_when_it_fits() {
        // "aaaa bbbb." closes on its period, leaving "cc." as a 3-unit tail. Under a 12-token ceiling
        // folding it back would need 14, so it stays alone -- and reads row 3, a different voice
        // cadence for three phonemes. This is what the fold exists to prevent.
        let stuck = plan_pieces(
            &words("aaaa bbbb. cc."),
            &PlanPolicy { max_units: 12, min_chunk_units: 5 },
        )
        .unwrap();
        assert_eq!(stuck.len(), 2, "{:?}", stuck.style_rows());
        assert_eq!(stuck.style_rows(), vec![10, 3], "the tail keeps a row of its own");

        // Same input, room to fold: one utterance, one row, no orphan prosody.
        let folded = plan_pieces(
            &words("aaaa bbbb. cc."),
            &PlanPolicy { max_units: 20, min_chunk_units: 5 },
        )
        .unwrap();
        assert_eq!(folded.len(), 1, "{:?}", folded.style_rows());
        assert_eq!(folded.style_rows(), vec![14], "10 + a space + 3");
    }
}
