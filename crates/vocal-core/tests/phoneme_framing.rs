//! The arithmetic a consumer has to get right, pinned beside the function that defines it.
//!
//! `apps/sample-reader` checks these same relations end-to-end, through the public API. This file
//! exists so that if `encode` or the planner ever changes shape, it fails in the crate that owns them
//! and says which relation broke, rather than surfacing as a wrong number in a sample's report line --
//! or, worse, as nothing at all in a build where every caller already assumed the new shape.

use vocal_core::phoneme_tokens::{encode, strip_to_vocab, MAX_TOKENS, SYMBOLS};
use vocal_core::utterance_plan::{plan_pieces, Piece, PlanPolicy};

/// A character the vocabulary does not carry, found rather than remembered: the ASCII `g` that this
/// table lacks is a fact about the current 178 symbols, and hard-coding it would let the test rot into
/// a tautology the day someone widens the table.
fn a_character_the_table_lacks() -> char {
    (0x201u32..0x2fffu32)
        .filter_map(char::from_u32)
        .find(|c| !SYMBOLS.contains(c))
        .expect(
            "some code point in the general punctuation and IPA ranges must be absent from a \
             178-symbol table",
        )
}

#[test]
fn encode_frames_its_input_with_exactly_two_ids() {
    let foreign = a_character_the_table_lacks();
    let with_foreign = format!("g{foreign}æʊ");
    for phonemes in ["a", "hələʊ wɜːld", "ˈtʃɪti ˈvəʊkəl rʌnˈtaɪm", with_foreign.as_str()] {
        assert_eq!(
            encode(phonemes).len(),
            phonemes.chars().count() + 2,
            "encode must stay PAD-front, content, PAD-back: a caller sizing input_ids from the \
             content count alone is one row short, which is why the sample prints units and framed \
             separately rather than one of them (input: {phonemes:?})"
        );
    }
}

#[test]
fn an_unmapped_symbol_costs_a_slot_instead_of_disappearing() {
    let foreign = a_character_the_table_lacks();
    let with_foreign = format!("æʊ{foreign}eɪ");
    assert_eq!(
        encode(&with_foreign).len(),
        with_foreign.chars().count() + 2,
        "a symbol the table lacks must occupy one slot, so length is preserved"
    );
    let stripped = strip_to_vocab(&with_foreign);
    assert!(
        stripped.chars().count() < with_foreign.chars().count(),
        "strip_to_vocab must remove what encode only pads -- that difference is the whole reason it \
         is a reporting helper and not part of the synthesis path, so if they ever agree the docs are \
         describing two names for one function"
    );
}

#[test]
fn a_long_input_saturates_at_the_cap_rather_than_measuring_the_plan() {
    let long = "ə".repeat(MAX_TOKENS + 400);
    assert_eq!(
        encode(&long).len(),
        MAX_TOKENS,
        "encode truncates, so a whole line's encoded length says nothing about a long input; a \
         consumer must count units per utterance, which is what plan_pieces bounds by the policy"
    );
}

#[test]
fn a_planned_utterance_counts_units_and_reads_its_own_row() {
    let policy = PlanPolicy {
        max_units: 509,
        min_chunk_units: 8,
    };
    policy.validate().expect("a policy inside the window is valid");
    let words = ["hələʊ", "wɜːld", "əv", "ˈaʊdʒoʊ", "ɪz", "ˈhɪə", "tə", "riːd"];
    let pieces: Vec<Piece> = words.iter().map(|word| Piece::phonemes(*word)).collect();
    let plan = plan_pieces(&pieces, &policy).expect("eight short words fit one utterance");
    assert!(
        !plan.utterances.is_empty(),
        "a non-empty input must plan into at least one utterance"
    );
    let planned_units: usize = plan.utterances.iter().map(|u| u.units).sum();
    let expected: usize = words.iter().map(|w| w.chars().count()).sum::<usize>() + words.len() - 1;
    assert_eq!(
        planned_units, expected,
        "units are the joined phonemes, single spaces included -- spaces are content the model sees"
    );
    for utterance in &plan.utterances {
        assert_eq!(
            encode(&utterance.phonemes).len(),
            utterance.units + 2,
            "an utterance must be encodable to exactly its own unit count plus the framing"
        );
        assert_eq!(
            utterance.style_row, utterance.units,
            "the style row is selected by content length, so it must equal the units being fed"
        );
    }
}
