//! About me: the opt-in corpus test of the line text operations (task
//! P6-01). For every line of every real song in the `USDX_CORPUS` folders it
//! checks that the display text, parsed back unchanged, plans no change at
//! all (leading spaces, two words on one note, a `~` inside a note and empty
//! texts included), and that adding a word to the line plans without a
//! panic. It is ignored by default; `scripts/dev.sh corpus` sets
//! `USDX_CORPUS` and runs it with `--ignored`. Without the variable it skips.

use std::path::{Path, PathBuf};

use usdx_edit::EditError;
use usdx_edit::text::{
    NoteStatus, line_notes, line_text_for_editing, parse_line_text, plan_line_text,
};
use usdx_format::bytes::decode;
use usdx_format::rows::{Document, RowKind, classify};
use usdx_format::song::Song;

/// song_documents walks a corpus folder and returns the documents of the
/// song files: `.txt` files with a `#BPM` header that are not Factory helper
/// files.
fn song_documents(root: &Path) -> Vec<(PathBuf, Document)> {
    let helpers = ["lyrics.txt", "language.txt", "song_end.txt", "anchors.txt"];
    let mut songs = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("{} is not readable: {error}", dir.display()));
        for entry in entries {
            let path = entry.expect("the corpus entry reads").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_lowercase();
            if !name.ends_with(".txt") || helpers.contains(&name.as_str()) {
                continue;
            }
            let bytes = std::fs::read(&path)
                .unwrap_or_else(|error| panic!("{} is not readable: {error}", path.display()));
            let Ok(file) = decode(&bytes) else {
                continue;
            };
            let doc = classify(file);
            let has_bpm = doc.rows.iter().any(
                |row| matches!(&row.kind, RowKind::Header { key, value } if key == "BPM" && !value.is_empty()),
            );
            if has_bpm {
                songs.push((path, doc));
            }
        }
    }
    songs.sort_by(|left, right| left.0.cmp(&right.0));
    songs
}

#[test]
#[ignore = "run by scripts/dev.sh corpus"]
fn every_corpus_line_plans_no_change_for_its_own_text() {
    let Some(corpus) = std::env::var_os("USDX_CORPUS") else {
        eprintln!("skipped: USDX_CORPUS is not set");
        return;
    };
    let (mut songs, mut lines, mut textless) = (0, 0, 0);
    for folder in std::env::split_paths(&corpus) {
        for (path, doc) in song_documents(&folder) {
            let song = Song::from_document(&doc);
            for (voice, line_index, line) in song.voices.iter().flat_map(|voice| {
                voice
                    .lines
                    .iter()
                    .enumerate()
                    .map(move |(index, line)| (voice.number, index, line))
            }) {
                let notes = line_notes(&doc, &line.notes).expect("a line lists notes");
                let display = line_text_for_editing(&notes);
                let syllables = parse_line_text(&display);
                if syllables.is_empty() {
                    textless += 1;
                    continue;
                }
                let place = format!("{} voice {voice} line {}", path.display(), line_index + 1);
                let plan = plan_line_text(&notes, &syllables, &[])
                    .unwrap_or_else(|error| panic!("{place}: {error}"));
                assert!(
                    plan.changes.is_empty(),
                    "{place}: {display:?} {:?}",
                    plan.changes
                );
                assert_eq!(plan.notes.len(), notes.len(), "{place}");
                for (planned, note) in plan.notes.iter().zip(&notes) {
                    assert_eq!(&planned.note, note, "{place}: {display:?}");
                    assert_eq!(planned.status, NoteStatus::Unchanged, "{place}");
                }
                let longer = parse_line_text(&format!("{display} extra"));
                match plan_line_text(&notes, &longer, &[]) {
                    Ok(_) | Err(EditError::NotesTooShort) => {}
                    Err(error) => panic!("{place}: adding a word fails: {error}"),
                }
                lines += 1;
            }
            songs += 1;
        }
    }
    assert!(
        songs >= 136,
        "the corpus folders yielded only {songs} songs"
    );
    println!("planned {lines} lines of {songs} songs unchanged ({textless} lines without text)");
}
