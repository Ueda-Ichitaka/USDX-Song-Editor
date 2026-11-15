//! About me: tests for the line text operations (task P6-01, plan section
//! 6.5): the syllable parser (spaces, `|`, `~`, punctuation), the display
//! text `line_text_for_editing` and its round trip, and the text plan with
//! its count fixes. User decisions 2026-11-15: a count difference is fixed
//! locally, inside the stretch of notes whose syllables changed ("Local to
//! the edit"); edited syllables always carry the trailing space ("Always
//! trailing space"); the `Ask` policy returns the proposal with one note
//! pick per count change ("Pick note per change"). Unchanged syllables keep
//! their notes and their exact text, and moving a space is no change.

use proptest::prelude::*;
use usdx_edit::EditError;
use usdx_edit::text::*;
use usdx_format::bytes::decode;
use usdx_format::rows::{Document, NoteRow, NoteType, RowId, RowKind, classify};
use usdx_format::song::Song;

/// texts lists the syllable texts.
fn texts(syllables: &[Syllable]) -> Vec<&str> {
    syllables
        .iter()
        .map(|syllable| syllable.text.as_str())
        .collect()
}

/// note builds a normal note row.
fn note(start: i64, length: i64, pitch: i64, text: &str) -> NoteRow {
    NoteRow {
        kind: NoteType::Normal,
        start,
        length,
        pitch,
        text: text.to_owned(),
    }
}

/// song_text builds a one-line song from `(start, length, pitch, text)`.
fn song_text(notes: &[(i64, i64, i64, &str)]) -> String {
    let mut text = String::from("#TITLE:T\n#ARTIST:A\n#BPM:300\n#GAP:0\n");
    for (start, length, pitch, syllable) in notes {
        text.push_str(&format!(": {start} {length} {pitch} {syllable}\n"));
    }
    text.push_str("E\n");
    text
}

/// loaded classifies a song text and returns the document, the song view and
/// the note ids of the first line.
fn loaded(text: &str) -> (Document, Song, Vec<RowId>) {
    let doc = classify(decode(text.as_bytes()).expect("the text decodes"));
    let song = Song::from_document(&doc);
    let ids = song.voices[0].lines[0].notes.clone();
    (doc, song, ids)
}

/// notes_of lists `(start, length, pitch, text)` of every note row.
fn notes_of(doc: &Document) -> Vec<(i64, i64, i64, String)> {
    doc.rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::Note(note) => Some((note.start, note.length, note.pitch, note.text.clone())),
            _ => None,
        })
        .collect()
}

/// applied plans the edit with the policy, applies the patch to a copy and
/// returns the copy.
fn applied(text: &str, edited: &str) -> Document {
    let (doc, song, ids) = loaded(text);
    let patch = plan_text_change(
        &doc,
        &song,
        &ids,
        &parse_line_text(edited),
        CountPolicy::SplitMerge,
    )
    .expect("the edit plans");
    let mut copy = doc.clone();
    patch.apply(&mut copy).expect("the patch applies");
    copy
}

/// The current line of the reference song (aequitaS - Realize The Real Lies,
/// line 3), as in the mockup and the plan's line text example.
const REFERENCE: &[(i64, i64, i64, &str)] = &[
    (161, 2, 7, "Are "),
    (165, 4, 9, "you "),
    (172, 13, 7, "free "),
    (189, 2, 7, "or "),
    (192, 10, 7, "just "),
    (206, 1, 5, "a "),
    (209, 16, 7, "slave "),
    (228, 2, 5, "of "),
    (232, 5, 7, "the"),
    (237, 6, 4, "~ "),
    (246, 8, 5, "au"),
    (254, 25, 7, "thorities? "),
];

const REFERENCE_TEXT: &str = "Are you free or just a slave of the~ au|thorities?";

// ---- parse_line_text ----

#[test]
fn a_space_separates_words_and_each_word_ends_with_a_space() {
    assert_eq!(texts(&parse_line_text("Are you")), vec!["Are ", "you "]);
}

#[test]
fn a_bar_splits_a_word_into_syllables() {
    assert_eq!(
        texts(&parse_line_text("au|tho|ri|ties?")),
        vec!["au", "tho", "ri", "ties? "]
    );
}

#[test]
fn a_tilde_starts_a_new_syllable_and_stays_in_its_text() {
    assert_eq!(texts(&parse_line_text("the~ au")), vec!["the", "~ ", "au "]);
    assert_eq!(texts(&parse_line_text("wo~rld")), vec!["wo", "~rld "]);
    assert_eq!(texts(&parse_line_text("oh ~")), vec!["oh ", "~ "]);
    assert_eq!(texts(&parse_line_text("ah~~")), vec!["ah", "~", "~ "]);
}

#[test]
fn surrounding_and_repeated_white_space_is_one_separator() {
    assert_eq!(
        texts(&parse_line_text("  Are \t you  ")),
        vec!["Are ", "you "]
    );
}

#[test]
fn punctuation_stays_in_the_syllable() {
    assert_eq!(
        texts(&parse_line_text("Hey, you! (oh) don't")),
        vec!["Hey, ", "you! ", "(oh) ", "don't "]
    );
}

#[test]
fn empty_pieces_give_no_syllable() {
    assert_eq!(texts(&parse_line_text("a||b |c|")), vec!["a", "b ", "c "]);
    assert!(parse_line_text("").is_empty());
    assert!(parse_line_text("  | ").is_empty());
}

// ---- line_text_for_editing ----

#[test]
fn the_display_text_joins_words_with_spaces_and_syllables_with_bars() {
    assert_eq!(
        line_text_for_editing(&[note(0, 1, 0, "au"), note(1, 1, 0, "thorities? ")]),
        "au|thorities?"
    );
    assert_eq!(
        line_text_for_editing(&[note(0, 1, 0, "Are "), note(1, 1, 0, "you ")]),
        "Are you"
    );
}

#[test]
fn a_held_note_follows_without_a_bar() {
    assert_eq!(
        line_text_for_editing(&[note(0, 1, 0, "the"), note(1, 1, 0, "~ ")]),
        "the~"
    );
    assert_eq!(
        line_text_for_editing(&[note(0, 1, 0, "wo"), note(1, 1, 0, "~rld ")]),
        "wo~rld"
    );
}

#[test]
fn leading_spaces_read_like_trailing_ones() {
    assert_eq!(
        line_text_for_editing(&[
            note(0, 1, 0, "Are"),
            note(1, 1, 0, " you"),
            note(2, 1, 0, " free")
        ]),
        "Are you free"
    );
    assert_eq!(
        line_text_for_editing(&[note(0, 1, 0, "free "), note(1, 1, 0, " or")]),
        "free or"
    );
}

#[test]
fn the_reference_line_reads_as_in_the_plan() {
    let notes: Vec<NoteRow> = REFERENCE
        .iter()
        .map(|(start, length, pitch, text)| note(*start, *length, *pitch, text))
        .collect();
    assert_eq!(line_text_for_editing(&notes), REFERENCE_TEXT);
}

#[test]
fn syllables_join_in_the_line_text_syntax() {
    assert_eq!(
        syllables_text(&parse_line_text("tho|ri|ties? ok the~")),
        "tho|ri|ties? ok the~"
    );
    assert_eq!(syllables_text(&[]), "");
}

#[test]
fn trailing_space_lines_round_trip_exactly() {
    let notes: Vec<NoteRow> = REFERENCE
        .iter()
        .map(|(start, length, pitch, text)| note(*start, *length, *pitch, text))
        .collect();
    let parsed = parse_line_text(&line_text_for_editing(&notes));
    let originals: Vec<&str> = notes.iter().map(|note| note.text.as_str()).collect();
    assert_eq!(texts(&parsed), originals);
}

// ---- plan_text_change: unchanged and same count ----

#[test]
fn an_unchanged_text_changes_nothing() {
    let (doc, song, ids) = loaded(&song_text(REFERENCE));
    let patch = plan_text_change(
        &doc,
        &song,
        &ids,
        &parse_line_text(REFERENCE_TEXT),
        CountPolicy::Ask,
    )
    .expect("the edit plans");
    assert!(patch.ops.is_empty());
}

#[test]
fn notes_the_syntax_cannot_express_survive_an_unchanged_text() {
    // Leading spaces, two words on one note, a tilde inside a note and an
    // empty note text: the display text cannot reproduce them, so an
    // unchanged display text must leave them as they are.
    let notes = [
        (0, 2, 0, "Oh"),
        (2, 2, 0, " oh ye"),
        (4, 2, 0, " he~"),
        (6, 2, 0, ""),
        (8, 2, 0, " DIE! ~ ~ "),
    ];
    let (doc, song, ids) = loaded(&song_text(&notes));
    let display = line_text_for_editing(
        &ids.iter()
            .map(|id| match &doc.rows[id.0 as usize].kind {
                RowKind::Note(note) => note.clone(),
                _ => unreachable!("the line lists notes"),
            })
            .collect::<Vec<_>>(),
    );
    let patch = plan_text_change(
        &doc,
        &song,
        &ids,
        &parse_line_text(&display),
        CountPolicy::Ask,
    )
    .expect("the edit plans");
    assert!(patch.ops.is_empty(), "{display:?} changed {:?}", patch.ops);
}

#[test]
fn moving_a_space_is_no_change() {
    let (doc, song, ids) = loaded(&song_text(&[(0, 2, 0, "Are"), (2, 2, 0, " you")]));
    let patch = plan_text_change(
        &doc,
        &song,
        &ids,
        &parse_line_text("Are you"),
        CountPolicy::Ask,
    )
    .expect("the edit plans");
    assert!(patch.ops.is_empty());
}

#[test]
fn a_changed_syllable_changes_only_its_note() {
    let out = applied(
        &song_text(REFERENCE),
        "Are you free or just a slaves of the~ au|thorities?",
    );
    let before = notes_of(&loaded(&song_text(REFERENCE)).0);
    let after = notes_of(&out);
    assert_eq!(after.len(), before.len());
    for (index, (old, new)) in before.iter().zip(&after).enumerate() {
        if index == 6 {
            assert_eq!(new, &(209, 16, 7, "slaves ".to_owned()));
        } else {
            assert_eq!(new, old, "note {index}");
        }
    }
    let dirty: Vec<u32> = out
        .rows
        .iter()
        .filter(|row| row.dirty)
        .map(|row| row.id.0)
        .collect();
    assert_eq!(dirty, vec![4 + 6]);
}

#[test]
fn a_changed_word_keeps_a_note_with_two_words() {
    let out = applied(
        &song_text(&[(0, 2, 0, "Oh "), (2, 2, 0, "oh ye "), (4, 2, 0, "end ")]),
        "Oh oh yeah end",
    );
    assert_eq!(
        notes_of(&out),
        vec![
            (0, 2, 0, "Oh ".to_owned()),
            (2, 2, 0, "oh yeah ".to_owned()),
            (4, 2, 0, "end ".to_owned()),
        ]
    );
}

#[test]
fn an_edited_word_after_a_leading_space_note_keeps_its_word_space() {
    // Songs that put the word space first ("All", " the", " world"): the
    // edited last word must stay a separate word, so it takes a space in
    // front where the unchanged note before it has none at its end
    // (Metric - Gold Guns Girls, Toto - Africa in the corpus sweep).
    let out = applied(
        &song_text(&[(0, 2, 0, "All"), (2, 2, 0, " the"), (4, 2, 0, " world")]),
        "All the worldzz",
    );
    assert_eq!(notes_of(&out)[2].3, " worldzz ");
    assert_eq!(
        line_text_for_editing(&[
            note(0, 2, 0, "All"),
            note(2, 2, 0, " the"),
            note(4, 2, 0, " worldzz ")
        ]),
        "All the worldzz"
    );
}

#[test]
fn an_edited_word_before_a_leading_space_note_gets_no_second_space() {
    // The unchanged note after the edit brings its own space, so the edited
    // word ends without one: exactly one space between the words.
    let out = applied(
        &song_text(&[(0, 2, 0, "All"), (2, 2, 0, " the"), (4, 2, 0, " world")]),
        "All THE world",
    );
    assert_eq!(
        notes_of(&out),
        vec![
            (0, 2, 0, "All".to_owned()),
            (2, 2, 0, " THE".to_owned()),
            (4, 2, 0, " world".to_owned()),
        ]
    );
}

#[test]
fn an_edit_inside_a_word_adds_no_space() {
    let out = applied(
        &song_text(&[(0, 2, 0, "Hel"), (2, 2, 0, "lo "), (4, 2, 0, " you")]),
        "Hel|LO you",
    );
    assert_eq!(notes_of(&out)[1].3, "LO");
}

// ---- plan_text_change: more syllables ----

#[test]
fn an_extra_syllable_splits_the_changed_note_in_halves() {
    let out = applied(
        &song_text(REFERENCE),
        "Are you free or just a slave of the~ au|tho|rities?",
    );
    let after = notes_of(&out);
    assert_eq!(after.len(), REFERENCE.len() + 1);
    assert_eq!(after[11], (254, 12, 7, "tho".to_owned()));
    assert_eq!(after[12], (266, 13, 7, "rities? ".to_owned()));
    // Every other note keeps its place and text.
    for (index, (start, length, pitch, text)) in REFERENCE[..11].iter().enumerate() {
        assert_eq!(
            after[index],
            (*start, *length, *pitch, (*text).to_owned()),
            "note {index}"
        );
    }
}

#[test]
fn the_split_is_local_to_the_edit_not_the_longest_note_of_the_line() {
    // The longest note of the line is the last one (25 beats); the extra
    // syllable is in the first word, so the first note splits.
    let out = applied(
        &song_text(REFERENCE),
        "A|re you free or just a slave of the~ au|thorities?",
    );
    let after = notes_of(&out);
    assert_eq!(after[0], (161, 1, 7, "A".to_owned()));
    assert_eq!(after[1], (162, 1, 7, "re ".to_owned()));
    assert_eq!(after[12], (254, 25, 7, "thorities? ".to_owned()));
}

#[test]
fn a_split_keeps_the_note_type_and_the_total_beats() {
    let (doc, song, ids) = loaded("#TITLE:T\n#BPM:300\n* 10 9 3 golden\nE\n");
    let patch = plan_text_change(
        &doc,
        &song,
        &ids,
        &parse_line_text("gol|den"),
        CountPolicy::SplitMerge,
    )
    .expect("the edit plans");
    let mut out = doc.clone();
    patch.apply(&mut out).expect("the patch applies");
    let kinds: Vec<NoteType> = out
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::Note(note) => Some(note.kind.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, vec![NoteType::Golden, NoteType::Golden]);
    let after = notes_of(&out);
    assert_eq!(after[0], (10, 4, 3, "gol".to_owned()));
    assert_eq!(after[1], (14, 5, 3, "den ".to_owned()));
    assert_eq!(after[0].1 + after[1].1, 9);
}

#[test]
fn two_extra_syllables_halve_the_longest_piece_again() {
    let out = applied(&song_text(&[(0, 25, 0, "thorities? ")]), "tho|ri|ties?");
    assert_eq!(
        notes_of(&out),
        vec![
            (0, 12, 0, "tho".to_owned()),
            (12, 6, 0, "ri".to_owned()),
            (18, 7, 0, "ties? ".to_owned()),
        ]
    );
}

#[test]
fn an_inserted_word_splits_the_note_before_it() {
    let out = applied(
        &song_text(&[(0, 8, 0, "Are "), (10, 4, 0, "free ")]),
        "Are you free",
    );
    assert_eq!(
        notes_of(&out),
        vec![
            (0, 4, 0, "Are ".to_owned()),
            (4, 4, 0, "you ".to_owned()),
            (10, 4, 0, "free ".to_owned()),
        ]
    );
}

#[test]
fn a_word_inserted_at_the_line_start_splits_the_first_note() {
    let out = applied(
        &song_text(&[(0, 8, 0, "free "), (10, 4, 0, "now ")]),
        "so free now",
    );
    assert_eq!(
        notes_of(&out),
        vec![
            (0, 4, 0, "so ".to_owned()),
            (4, 4, 0, "free ".to_owned()),
            (10, 4, 0, "now ".to_owned()),
        ]
    );
}

#[test]
fn a_note_too_short_for_the_extra_syllables_is_refused() {
    let (doc, song, ids) = loaded(&song_text(&[(0, 1, 0, "a "), (1, 4, 0, "b ")]));
    let result = plan_text_change(
        &doc,
        &song,
        &ids,
        &parse_line_text("x|y b"),
        CountPolicy::SplitMerge,
    );
    assert_eq!(result.unwrap_err(), EditError::NotesTooShort);
}

// ---- plan_text_change: fewer syllables ----

#[test]
fn a_lost_syllable_merges_the_changed_notes() {
    let out = applied(
        &song_text(&[(0, 2, 0, "beau"), (3, 2, 0, "ti"), (6, 3, 0, "ful ")]),
        "beau|tiful",
    );
    assert_eq!(
        notes_of(&out),
        vec![(0, 2, 0, "beau".to_owned()), (3, 6, 0, "tiful ".to_owned())]
    );
}

#[test]
fn a_deleted_word_merges_its_note_into_the_note_before() {
    let out = applied(
        &song_text(&[
            (161, 2, 7, "Are "),
            (165, 4, 9, "you "),
            (172, 13, 7, "free "),
        ]),
        "Are free",
    );
    assert_eq!(
        notes_of(&out),
        vec![
            (161, 8, 7, "Are ".to_owned()),
            (172, 13, 7, "free ".to_owned())
        ]
    );
}

#[test]
fn the_shortest_neighbouring_pair_merges() {
    // "ab" "cd" "ef" all change and become two syllables: the pair with
    // the shortest merged span (cd + ef: 3 beats) merges.
    let out = applied(
        &song_text(&[(0, 4, 0, "ab "), (4, 1, 0, "cd "), (5, 2, 0, "ef ")]),
        "AB CDEF",
    );
    assert_eq!(
        notes_of(&out),
        vec![(0, 4, 0, "AB ".to_owned()), (4, 3, 0, "CDEF ".to_owned())]
    );
}

#[test]
fn an_empty_line_text_is_refused() {
    let (doc, song, ids) = loaded(&song_text(REFERENCE));
    let result = plan_text_change(&doc, &song, &ids, &parse_line_text(" | "), CountPolicy::Ask);
    assert_eq!(result.unwrap_err(), EditError::EmptyLineText);
}

// ---- the Ask policy and note picks ----

#[test]
fn the_ask_policy_returns_the_proposal() {
    let (doc, song, ids) = loaded(&song_text(REFERENCE));
    let result = plan_text_change(
        &doc,
        &song,
        &ids,
        &parse_line_text("Are you free or just a slave of the~ au|tho|ri|ties?"),
        CountPolicy::Ask,
    );
    let Err(EditError::NeedsDecision(plan)) = result else {
        panic!("the Ask policy needs a decision: {result:?}");
    };
    assert_eq!(plan.changes.len(), 1);
    let change = &plan.changes[0];
    assert_eq!(change.kind, CountFix::Split);
    assert_eq!(change.notes, 11..12);
    assert_eq!(change.syllables, 11..14);
    assert_eq!(change.candidates, vec![11]);
    assert_eq!(change.chosen, 11);
    // The proposal holds the notes the automatic fix gives.
    assert_eq!(plan.notes.len(), 14);
    assert_eq!(plan.notes[11].note.text, "tho");
    assert_eq!(plan.notes[11].status, NoteStatus::Changed);
    assert_eq!(plan.notes[12].status, NoteStatus::New);
    assert_eq!(plan.notes[13].status, NoteStatus::New);
    assert_eq!(plan.notes[0].status, NoteStatus::Unchanged);
}

#[test]
fn the_ask_policy_without_a_count_difference_gives_the_patch() {
    let (doc, song, ids) = loaded(&song_text(REFERENCE));
    let patch = plan_text_change(
        &doc,
        &song,
        &ids,
        &parse_line_text("Are you free or just a slaves of the~ au|thorities?"),
        CountPolicy::Ask,
    )
    .expect("same counts need no decision");
    assert_eq!(patch.ops.len(), 1);
    assert_eq!(patch.label, "Edit line text");
}

#[test]
fn a_pick_splits_the_chosen_note_instead_of_the_longest() {
    let notes = [(0, 6, 0, "ab "), (6, 2, 0, "cd ")];
    let rows: Vec<NoteRow> = notes
        .iter()
        .map(|(start, length, pitch, text)| note(*start, *length, *pitch, text))
        .collect();
    let syllables = parse_line_text("AB CD EF");
    let automatic = plan_line_text(&rows, &syllables, &[]).expect("the edit plans");
    assert_eq!(automatic.changes[0].candidates, vec![0, 1]);
    assert_eq!(automatic.changes[0].chosen, 0);
    let picked = plan_line_text(&rows, &syllables, &[Some(1)]).expect("the edit plans");
    assert_eq!(picked.changes[0].chosen, 1);
    let planned: Vec<(i64, i64, &str)> = picked
        .notes
        .iter()
        .map(|planned| {
            (
                planned.note.start,
                planned.note.length,
                planned.note.text.as_str(),
            )
        })
        .collect();
    assert_eq!(planned, vec![(0, 6, "AB "), (6, 1, "CD "), (7, 1, "EF ")]);

    let (doc, song, ids) = loaded(&song_text(&notes));
    let patch = text_change_patch(&doc, &song, &ids, &picked).expect("the plan patches");
    let mut out = doc.clone();
    patch.apply(&mut out).expect("the patch applies");
    assert_eq!(
        notes_of(&out),
        vec![
            (0, 6, 0, "AB ".to_owned()),
            (6, 1, 0, "CD ".to_owned()),
            (7, 1, 0, "EF ".to_owned()),
        ]
    );
}

#[test]
fn a_pick_merges_the_chosen_pair() {
    let rows = vec![
        note(0, 4, 0, "ab "),
        note(4, 1, 0, "cd "),
        note(5, 2, 0, "ef "),
    ];
    let syllables = parse_line_text("AB CDEF");
    let automatic = plan_line_text(&rows, &syllables, &[]).expect("the edit plans");
    assert_eq!(automatic.changes[0].kind, CountFix::Merge);
    assert_eq!(automatic.changes[0].candidates, vec![0, 1]);
    assert_eq!(automatic.changes[0].chosen, 1);
    let picked = plan_line_text(&rows, &syllables, &[Some(0)]).expect("the edit plans");
    let planned: Vec<(i64, i64, &str)> = picked
        .notes
        .iter()
        .map(|planned| {
            (
                planned.note.start,
                planned.note.length,
                planned.note.text.as_str(),
            )
        })
        .collect();
    assert_eq!(planned, vec![(0, 5, "AB "), (5, 2, "CDEF ")]);
}

#[test]
fn a_pick_outside_the_candidates_falls_back_to_the_automatic_choice() {
    let rows = vec![note(0, 6, 0, "ab "), note(6, 2, 0, "cd ")];
    let syllables = parse_line_text("AB CD EF");
    let picked = plan_line_text(&rows, &syllables, &[Some(7)]).expect("the edit plans");
    assert_eq!(picked.changes[0].chosen, 0);
}

// ---- patches ----

#[test]
fn the_patch_undoes_exactly() {
    let (doc, song, ids) = loaded(&song_text(REFERENCE));
    let patch = plan_text_change(
        &doc,
        &song,
        &ids,
        &parse_line_text("Are free or just a slave of the~ au|tho|ri|ties? yes"),
        CountPolicy::SplitMerge,
    )
    .expect("the edit plans");
    let mut out = doc.clone();
    patch.apply(&mut out).expect("the patch applies");
    assert_ne!(out, doc);
    patch
        .inverse()
        .apply(&mut out)
        .expect("the inverse applies");
    assert_eq!(out, doc);
}

#[test]
fn relative_songs_take_text_changes_but_refuse_count_changes() {
    let relative =
        "#TITLE:T\n#BPM:300\n#RELATIVE:yes\n: 0 4 0 ab \n: 4 4 1 cd \n- 8 8\n: 0 4 2 ef \nE\n";
    let (doc, song, ids) = loaded(relative);
    let same = plan_text_change(
        &doc,
        &song,
        &ids,
        &parse_line_text("AB CD"),
        CountPolicy::SplitMerge,
    );
    assert!(same.is_ok(), "{same:?}");
    let more = plan_text_change(
        &doc,
        &song,
        &ids,
        &parse_line_text("AB CD EF"),
        CountPolicy::SplitMerge,
    );
    assert_eq!(more.unwrap_err(), EditError::RelativeModeIsReadOnly);
}

// ---- properties ----

/// note_text generates note texts with the characters that matter to the
/// syntax: letters, spaces, tabs, `~` and punctuation.
fn note_text() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[ a-c~,!\t]{0,6}").expect("the text regex compiles")
}

proptest! {
    #[test]
    fn an_unchanged_display_text_never_changes_a_note(
        texts in proptest::collection::vec(note_text(), 1..8)
    ) {
        let notes: Vec<(i64, i64, i64, &str)> = texts
            .iter()
            .enumerate()
            .map(|(index, text)| (index as i64 * 4, 4, 0, text.as_str()))
            .collect();
        let rows: Vec<NoteRow> = notes
            .iter()
            .map(|(start, length, pitch, text)| note(*start, *length, *pitch, text))
            .collect();
        let syllables = parse_line_text(&line_text_for_editing(&rows));
        prop_assume!(!syllables.is_empty());
        let plan = plan_line_text(&rows, &syllables, &[]).expect("the edit plans");
        prop_assert!(plan.changes.is_empty());
        for (planned, row) in plan.notes.iter().zip(&rows) {
            prop_assert_eq!(&planned.note, row);
            prop_assert_eq!(&planned.status, &NoteStatus::Unchanged);
        }
        prop_assert_eq!(plan.notes.len(), rows.len());
    }

    #[test]
    fn a_planned_line_keeps_the_syllables_in_order(
        texts in proptest::collection::vec("[a-c]{1,3}", 1..6),
        edited in proptest::collection::vec("[a-d]{1,3}", 1..8)
    ) {
        let rows: Vec<NoteRow> = texts
            .iter()
            .enumerate()
            .map(|(index, text)| note(index as i64 * 8, 8, 0, &format!("{text} ")))
            .collect();
        let syllables = parse_line_text(&edited.join(" "));
        let plan = plan_line_text(&rows, &syllables, &[]).expect("8-beat notes split");
        let planned: Vec<String> = plan
            .notes
            .iter()
            .map(|planned| planned.note.text.trim().to_owned())
            .collect();
        let wanted: Vec<String> = syllables
            .iter()
            .map(|syllable| syllable.text.trim().to_owned())
            .collect();
        prop_assert_eq!(planned, wanted);
        // The notes stay in order and never overlap.
        for pair in plan.notes.windows(2) {
            prop_assert!(pair[0].note.start + pair[0].note.length <= pair[1].note.start);
            prop_assert!(pair[0].note.length >= 1);
        }
    }
}
