//! About me: tests for the line text controller of task P6-02 (UI_SPEC 3.6,
//! 5.4, plan 8.3): the line text editor's field text, its preview (the
//! syllable chips with their states, the count message, the note picks of
//! "Ask me per note" and the refusals that disable Apply), the preview JSON
//! the bridge hands to QML, the apply as one undo step, and the inline
//! syllable edit over the lyric label (user decision 2026-11-15) with the
//! lyric hit test. The fixture's second line is the reference line of the
//! mockup (aequitaS - Realize The Real Lies, line 3).

use tempfile::TempDir;
use usdx_edit::text::CountFix;
use usdx_format::rows::{RowId, RowKind};
use usdx_song_editor::controller::EditorController;
use usdx_song_editor::controller::editing::SelectionMode;
use usdx_song_editor::controller::line_text::{
    Chip, ChipState, LineTextPreview, PickOption, parse_picks, preview_json,
};

/// Line 1 (rows 2-3, the second note with a leading space), its phrase
/// end (row 4), the reference line (rows 5-16), a phrase end (row 17) and
/// a one-note line 3 (row 18).
const SONG: &str = concat!(
    "#TITLE:T\n#BPM:300\n",
    ": 100 4 0 Hey\n: 105 4 0  there\n- 120\n",
    ": 161 2 7 Are \n: 165 4 9 you \n: 172 13 7 free \n: 189 2 7 or \n",
    ": 192 10 7 just \n: 206 1 5 a \n: 209 16 7 slave \n: 228 2 5 of \n",
    ": 232 5 7 the\n: 237 6 4 ~ \n: 246 8 5 au\n: 254 25 7 thorities? \n",
    "- 290\n: 300 4 0 end \nE\n",
);

const REFERENCE_TEXT: &str = "Are you free or just a slave of the~ au|thorities?";

/// controller loads the fixture from a temporary folder.
fn controller() -> (TempDir, EditorController) {
    let dir = tempfile::tempdir().expect("the temporary directory is created");
    let path = dir.path().join("song.txt");
    std::fs::write(&path, SONG).expect("the fixture is written");
    let mut controller = EditorController::new();
    controller.open(&path).expect("the song loads");
    (dir, controller)
}

/// line_notes lists `(start, length, text)` of the notes of a line.
fn line_notes(controller: &EditorController, line: usize) -> Vec<(i64, i64, String)> {
    let loaded = controller.loaded().expect("a song is loaded");
    controller
        .line_data(line)
        .expect("the line exists")
        .notes
        .iter()
        .map(|id| match &loaded.document.rows[id.0 as usize].kind {
            RowKind::Note(note) => (note.start, note.length, note.text.clone()),
            _ => unreachable!("a line lists notes"),
        })
        .collect()
}

/// note_text returns the text of the note row.
fn note_text(controller: &EditorController, id: RowId) -> String {
    match &controller.loaded().expect("a song is loaded").document.rows[id.0 as usize].kind {
        RowKind::Note(note) => note.text.clone(),
        _ => unreachable!("the row is a note"),
    }
}

/// undo_label returns the label of the last history step.
fn undo_label(controller: &EditorController) -> Option<String> {
    controller
        .loaded()
        .and_then(|loaded| loaded.history.undo_label().map(str::to_owned))
}

/// chip builds an expected chip.
fn chip(text: &str, beat: i64, state: ChipState) -> Chip {
    Chip {
        text: text.to_owned(),
        beat,
        state,
    }
}

// ---- The field text ----

#[test]
fn the_edit_field_shows_the_line_in_the_line_text_syntax() {
    let (_dir, controller) = controller();
    assert_eq!(controller.line_text(1).as_deref(), Some(REFERENCE_TEXT));
    assert_eq!(controller.line_text(0).as_deref(), Some("Hey there"));
    assert_eq!(controller.line_text(9), None);
}

// ---- The preview ----

#[test]
fn an_unchanged_text_previews_every_chip_unchanged_without_a_message() {
    let (_dir, controller) = controller();
    let preview = controller.preview_line_text(1, REFERENCE_TEXT, &[]);
    assert_eq!(preview.chips.len(), 12);
    assert_eq!(preview.chips[0], chip("Are", 161, ChipState::Same));
    assert_eq!(preview.chips[9], chip("~", 237, ChipState::Same));
    assert_eq!(preview.chips[11], chip("thorities?", 254, ChipState::Same));
    assert_eq!(preview.syllable_count, 12);
    assert_eq!(preview.note_count, 12);
    assert!(preview.changes.is_empty());
    assert_eq!(preview.message, "");
    assert_eq!(preview.error, "");
    assert!(!preview.changed);
    assert!(preview.can_apply);
}

#[test]
fn a_changed_syllable_shows_a_changed_chip() {
    let (_dir, controller) = controller();
    let preview = controller.preview_line_text(
        1,
        "Are you free or just a slaves of the~ au|thorities?",
        &[],
    );
    assert_eq!(preview.chips[6], chip("slaves", 209, ChipState::Changed));
    assert_eq!(preview.chips[7], chip("of", 228, ChipState::Same));
    assert!(preview.changed);
    assert_eq!(preview.message, "");
}

#[test]
fn one_extra_syllable_explains_the_split() {
    let (_dir, controller) = controller();
    let preview = controller.preview_line_text(
        1,
        "Are you free or just a slave of the~ au|tho|rities?",
        &[],
    );
    assert_eq!(preview.chips.len(), 13);
    assert_eq!(preview.chips[11], chip("tho", 254, ChipState::Changed));
    assert_eq!(preview.chips[12], chip("rities?", 266, ChipState::New));
    assert_eq!(
        preview.message,
        "13 syllables for 12 notes. \"tho|rities?\" now has two syllables on one note. \
         The editor will split \"thorities?\" (25 beats) into two notes of 12 and 13 beats \
         at the same pitch."
    );
    assert_eq!(preview.changes.len(), 1);
    assert_eq!(preview.changes[0].kind, CountFix::Split);
    assert_eq!(
        preview.changes[0].options,
        vec![PickOption {
            note: 11,
            label: "\"thorities?\" (25 beats)".to_owned()
        }]
    );
    assert_eq!(preview.changes[0].chosen, 11);
    assert!(preview.can_apply);
}

#[test]
fn two_extra_syllables_name_every_piece() {
    let (_dir, controller) = controller();
    let preview = controller.preview_line_text(
        1,
        "Are you free or just a slave of the~ au|tho|ri|ties?",
        &[],
    );
    assert_eq!(
        preview.chips[11..],
        [
            chip("tho", 254, ChipState::Changed),
            chip("ri", 266, ChipState::New),
            chip("ties?", 272, ChipState::New),
        ]
    );
    assert_eq!(
        preview.message,
        "14 syllables for 12 notes. \"tho|ri|ties?\" now has three syllables on one note. \
         The editor will split \"thorities?\" (25 beats) into three notes of 12, 6 and 7 \
         beats at the same pitch."
    );
}

#[test]
fn a_lost_word_explains_the_merge() {
    let (_dir, controller) = controller();
    let preview =
        controller.preview_line_text(1, "you free or just a slave of the~ au|thorities?", &[]);
    assert_eq!(preview.chips[0], chip("you", 161, ChipState::Changed));
    assert_eq!(
        preview.message,
        "11 syllables for 12 notes. \"you\" now has one syllable on two notes. \
         The editor will merge \"Are\" and \"you\" (8 beats) into one note."
    );
    assert_eq!(preview.changes[0].kind, CountFix::Merge);
    assert_eq!(
        preview.changes[0].options,
        vec![PickOption {
            note: 0,
            label: "\"Are\" + \"you\" (8 beats)".to_owned()
        }]
    );
}

#[test]
fn several_changes_explain_each() {
    let (_dir, controller) = controller();
    let preview =
        controller.preview_line_text(1, "you free or just a slave of the~ au|tho|rities?", &[]);
    assert_eq!(
        preview.message,
        "12 syllables for 12 notes. \"you\" now has one syllable on two notes. \
         The editor will merge \"Are\" and \"you\" (8 beats) into one note. \
         \"tho|rities?\" now has two syllables on one note. The editor will split \
         \"thorities?\" (25 beats) into two notes of 12 and 13 beats at the same pitch."
    );
    assert_eq!(preview.changes.len(), 2);
}

#[test]
fn a_note_too_short_disables_apply() {
    let (_dir, controller) = controller();
    let preview = controller.preview_line_text(
        1,
        "Are you free or just x|y slave of the~ au|thorities?",
        &[],
    );
    assert_eq!(
        preview.error,
        "The changed notes are too short for the added syllables: a note needs at least \
         1 beat per syllable."
    );
    assert!(!preview.can_apply);
    assert!(preview.chips.is_empty());
}

#[test]
fn an_empty_text_disables_apply() {
    let (_dir, controller) = controller();
    let preview = controller.preview_line_text(1, "  | ", &[]);
    assert_eq!(preview.error, "The line needs at least one syllable.");
    assert!(!preview.can_apply);
}

#[test]
fn a_pick_lists_every_candidate_and_moves_the_split() {
    let (_dir, controller) = controller();
    let text = "Are you free or just a slave of the~ AU|THO|RI|TIES?";
    let automatic = controller.preview_line_text(1, text, &[]);
    assert_eq!(
        automatic.changes[0].options,
        vec![
            PickOption {
                note: 10,
                label: "\"au\" (8 beats)".to_owned()
            },
            PickOption {
                note: 11,
                label: "\"thorities?\" (25 beats)".to_owned()
            },
        ]
    );
    assert_eq!(automatic.changes[0].chosen, 11);
    let picked = controller.preview_line_text(1, text, &[Some(10)]);
    assert_eq!(picked.changes[0].chosen, 10);
    assert_eq!(
        picked.chips[10..],
        [
            chip("AU", 246, ChipState::Changed),
            chip("THO", 248, ChipState::New),
            chip("RI", 250, ChipState::New),
            chip("TIES?", 254, ChipState::Changed),
        ]
    );
}

// ---- The preview JSON ----

#[test]
fn the_preview_json_carries_chips_changes_and_message() {
    let preview = LineTextPreview {
        chips: vec![
            chip("a\"b", 1, ChipState::Same),
            chip("c\\d", 2, ChipState::Changed),
            chip("e", 3, ChipState::New),
        ],
        changes: vec![usdx_song_editor::controller::line_text::ChangeChoice {
            kind: CountFix::Split,
            options: vec![PickOption {
                note: 4,
                label: "\"x\" (2 beats)".to_owned(),
            }],
            chosen: 4,
        }],
        syllable_count: 3,
        note_count: 2,
        message: "line\nbreak".to_owned(),
        error: String::new(),
        changed: true,
        can_apply: true,
    };
    assert_eq!(
        preview_json(&preview),
        concat!(
            r#"{"chips":[{"text":"a\"b","beat":1,"state":"same"},"#,
            r#"{"text":"c\\d","beat":2,"state":"changed"},"#,
            r#"{"text":"e","beat":3,"state":"new"}],"#,
            r#""changes":[{"kind":"split","options":[{"note":4,"label":"\"x\" (2 beats)"}],"chosen":4}],"#,
            r#""syllableCount":3,"noteCount":2,"message":"line\nbreak","error":"","#,
            r#""changed":true,"canApply":true}"#
        )
    );
}

#[test]
fn picks_parse_from_the_dialog_list() {
    assert_eq!(parse_picks("11,-1, 3"), vec![Some(11), None, Some(3)]);
    assert_eq!(parse_picks(""), Vec::<Option<usize>>::new());
    assert_eq!(parse_picks("x,2"), vec![None, Some(2)]);
}

// ---- Apply ----

#[test]
fn apply_commits_one_undo_step() {
    let (_dir, mut controller) = controller();
    let applied = controller
        .apply_line_text(
            1,
            "Are you free or just a slave of the~ au|tho|rities?",
            &[],
        )
        .expect("the text applies");
    assert!(applied);
    assert!(controller.is_dirty());
    assert_eq!(undo_label(&controller).as_deref(), Some("Edit line text"));
    let notes = line_notes(&controller, 1);
    assert_eq!(notes.len(), 13);
    assert_eq!(notes[11], (254, 12, "tho".to_owned()));
    assert_eq!(notes[12], (266, 13, "rities? ".to_owned()));
    // The other lines keep their notes.
    assert_eq!(
        line_notes(&controller, 2),
        vec![(300, 4, "end ".to_owned())]
    );
    controller.undo();
    assert_eq!(line_notes(&controller, 1).len(), 12);
    assert!(!controller.is_dirty());
}

#[test]
fn applying_an_unchanged_text_adds_no_undo_step() {
    let (_dir, mut controller) = controller();
    let applied = controller
        .apply_line_text(1, REFERENCE_TEXT, &[])
        .expect("the text applies");
    assert!(!applied);
    assert!(!controller.is_dirty());
    assert_eq!(undo_label(&controller), None);
}

#[test]
fn apply_honours_a_pick() {
    let (_dir, mut controller) = controller();
    controller
        .apply_line_text(
            1,
            "Are you free or just a slave of the~ AU|THO|RI|TIES?",
            &[Some(10)],
        )
        .expect("the text applies");
    let notes = line_notes(&controller, 1);
    assert_eq!(
        notes[10..],
        [
            (246, 2, "AU".to_owned()),
            (248, 2, "THO".to_owned()),
            (250, 4, "RI".to_owned()),
            (254, 25, "TIES? ".to_owned()),
        ]
    );
}

#[test]
fn a_count_change_clears_the_selection_and_a_text_change_keeps_it() {
    let (_dir, mut controller) = controller();
    controller.select_note(RowId(11), SelectionMode::Set);
    controller
        .apply_line_text(
            1,
            "Are you free or just a slaves of the~ au|thorities?",
            &[],
        )
        .expect("the text applies");
    assert_eq!(
        controller.selection().iter().copied().collect::<Vec<_>>(),
        vec![RowId(11)]
    );
    controller
        .apply_line_text(
            1,
            "Are you free or just a slave of the~ au|tho|rities?",
            &[],
        )
        .expect("the text applies");
    assert!(controller.selection().is_empty());
}

#[test]
fn a_refused_text_changes_nothing() {
    let (_dir, mut controller) = controller();
    let result = controller.apply_line_text(1, "", &[]);
    assert!(result.is_err());
    assert!(!controller.is_dirty());
}

// ---- The inline syllable edit ----

#[test]
fn the_inline_field_shows_the_trimmed_syllable() {
    let (_dir, controller) = controller();
    assert_eq!(
        controller.syllable_for_editing(RowId(3)).as_deref(),
        Some("there")
    );
    assert_eq!(
        controller.syllable_for_editing(RowId(5)).as_deref(),
        Some("Are")
    );
    assert_eq!(controller.syllable_for_editing(RowId(4)), None);
}

#[test]
fn an_inline_syllable_edit_keeps_the_word_space() {
    let (_dir, mut controller) = controller();
    let applied = controller
        .set_note_syllable(RowId(5), "Were")
        .expect("the syllable applies");
    assert!(applied);
    assert_eq!(note_text(&controller, RowId(5)), "Were ");
    assert_eq!(undo_label(&controller).as_deref(), Some("Edit syllable"));
}

#[test]
fn an_inline_edit_keeps_a_leading_space_leading() {
    let (_dir, mut controller) = controller();
    controller
        .set_note_syllable(RowId(3), "here")
        .expect("the syllable applies");
    assert_eq!(note_text(&controller, RowId(3)), " here");
}

#[test]
fn an_unchanged_or_empty_inline_edit_adds_no_step() {
    let (_dir, mut controller) = controller();
    assert!(
        !controller
            .set_note_syllable(RowId(5), "  Are ")
            .expect("no change")
    );
    assert!(
        !controller
            .set_note_syllable(RowId(5), "   ")
            .expect("no change")
    );
    assert!(!controller.is_dirty());
    assert_eq!(note_text(&controller, RowId(5)), "Are ");
}

// ---- The lyric hit test ----

#[test]
fn a_lyric_label_beat_finds_its_note() {
    let (_dir, controller) = controller();
    assert_eq!(controller.note_at_lyric_beat(170.0), Some(RowId(6)));
    assert_eq!(controller.note_at_lyric_beat(161.0), Some(RowId(5)));
    assert_eq!(controller.note_at_lyric_beat(500.0), Some(RowId(18)));
    assert_eq!(controller.note_at_lyric_beat(99.5), None);
}
