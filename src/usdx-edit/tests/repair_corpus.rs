//! About me: the opt-in corpus check of the timing repair (phase 7). For
//! every voice of every real song in the `USDX_CORPUS` folders it reads the
//! voice as it stands (`timed_lines`) and runs the collision pass over it,
//! and reports how many voices come out changed: a song that already holds
//! overlapping or stacked notes is "repaired" by the pass, which tells how
//! far a retime's collision pass may reach. It asserts only that the pass
//! never needs a #GAP shift for an unchanged song. It is ignored by
//! default; `scripts/dev.sh corpus` runs it.

use std::path::{Path, PathBuf};

use usdx_edit::collisions::{resolve_collisions, timed_lines};
use usdx_format::bytes::decode;
use usdx_format::rows::{Document, RowKind, classify};
use usdx_format::song::Song;

/// song_documents walks a corpus folder for the song files.
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
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let Ok(file) = decode(&bytes) else {
                continue;
            };
            let doc = classify(file);
            if doc.rows.iter().any(
                |row| matches!(&row.kind, RowKind::Header { key, value } if key == "BPM" && !value.is_empty()),
            ) {
                songs.push((path, doc));
            }
        }
    }
    songs.sort_by(|left, right| left.0.cmp(&right.0));
    songs
}

#[test]
#[ignore = "run by scripts/dev.sh corpus"]
fn the_collision_pass_over_unchanged_corpus_voices() {
    let Some(corpus) = std::env::var_os("USDX_CORPUS") else {
        eprintln!("skipped: USDX_CORPUS is not set");
        return;
    };
    let (mut voices, mut changed_voices, mut changed_notes, mut notes) = (0, 0, 0, 0);
    let mut examples = Vec::new();
    for folder in std::env::split_paths(&corpus) {
        for (path, doc) in song_documents(&folder) {
            let song = Song::from_document(&doc);
            if song.relative {
                continue;
            }
            for voice in 0..song.voices.len() {
                let lines = timed_lines(&doc, &song, voice);
                let placed = resolve_collisions(&lines).unwrap_or_else(|shift| {
                    panic!("{}: an unchanged voice asked for {shift:?}", path.display())
                });
                voices += 1;
                let mut changed_here = 0;
                for (line, placed_note) in lines
                    .iter()
                    .flat_map(|line| line.notes.iter())
                    .zip(&placed.notes)
                {
                    notes += 1;
                    let length = (line.end - line.start).round() as i64;
                    if placed_note.start != line.start.round() as i64
                        || placed_note.length != length
                    {
                        changed_here += 1;
                    }
                }
                if changed_here > 0 {
                    changed_voices += 1;
                    changed_notes += changed_here;
                    if examples.len() < 5 {
                        examples.push(format!("{} ({changed_here} notes)", path.display()));
                    }
                }
            }
        }
    }
    println!(
        "collision pass over unchanged voices: {changed_voices} of {voices} voices change, {changed_notes} of {notes} notes"
    );
    for example in examples {
        println!("  {example}");
    }
}
