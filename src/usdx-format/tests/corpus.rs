//! About me: the opt-in corpus test (task P1-09, plan sections 9.2 and 9.5).
//! It round-trips every real song file of the `USDX_CORPUS` folders
//! byte-identically. It is ignored by default; `scripts/dev.sh corpus` sets
//! `USDX_CORPUS` to the approved Factory output folders and runs it with
//! `--ignored`. Without the variable it skips on its own, so
//! `scripts/dev.sh check --release` stays green.

use std::path::{Path, PathBuf};

use usdx_format::bytes::{decode, encode};
use usdx_format::rows::classify;
use usdx_format::serialize::serialize;

/// is_helper_file tells whether a file name is one of the Factory's helper
/// files (any case), which are not songs.
fn is_helper_file(name: &str) -> bool {
    ["lyrics.txt", "language.txt", "song_end.txt", "anchors.txt"]
        .iter()
        .any(|helper| name.eq_ignore_ascii_case(helper))
}

/// is_song_file tells whether a .txt file is a song file: the Factory helper
/// files and .bak backups are never songs, and a song carries a `#BPM`
/// header.
fn is_song_file(path: &Path, bytes: &[u8]) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if name.to_lowercase().ends_with(".bak") || is_helper_file(name) {
        return false;
    }
    let Ok(file) = decode(bytes) else {
        return false;
    };
    classify(file)
        .rows
        .iter()
        .any(|row| matches!(&row.kind, usdx_format::rows::RowKind::Header { key, value } if key == "BPM" && !value.is_empty()))
}

/// collect_song_files walks a corpus folder recursively and returns the song
/// files with their bytes.
fn collect_song_files(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut songs = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).unwrap_or_else(|error| {
            panic!(
                "the corpus folder {} is not readable: {error}",
                dir.display()
            )
        });
        for entry in entries {
            let path = entry.expect("the corpus entry reads").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("txt") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap_or_else(|error| {
                panic!("the file {} is not readable: {error}", path.display())
            });
            if is_song_file(&path, &bytes) {
                songs.push((path, bytes));
            }
        }
    }
    songs.sort_by(|left, right| left.0.cmp(&right.0));
    songs
}

#[test]
#[ignore = "run by scripts/dev.sh corpus"]
fn every_corpus_song_round_trips_byte_identically() {
    let Some(corpus) = std::env::var_os("USDX_CORPUS") else {
        eprintln!("skipped: USDX_CORPUS is not set");
        return;
    };
    let mut total = 0;
    for folder in std::env::split_paths(&corpus) {
        let songs = collect_song_files(&folder);
        for (path, bytes) in &songs {
            let file = decode(bytes).unwrap_or_else(|error| {
                panic!("the song {} does not decode: {error}", path.display())
            });
            let doc = classify(file);
            let out = serialize(&doc).unwrap_or_else(|error| {
                panic!("the song {} does not serialize: {error}", path.display())
            });
            assert_eq!(
                out,
                *bytes,
                "the song {} does not round trip byte-identically",
                path.display()
            );
            // The lower-level encode round trip holds for the unmodified
            // document as well.
            let rebuilt = encode(&usdx_format::bytes::DecodedFile {
                bom: doc.file.bom,
                encoding: doc.file.encoding,
                encoding_source: doc.file.encoding_source,
                lines: doc
                    .rows
                    .iter()
                    .map(|row| row.raw.clone())
                    .chain(doc.trailing.iter().cloned())
                    .collect(),
            })
            .unwrap_or_else(|error| panic!("the song {} does not encode: {error}", path.display()));
            assert_eq!(
                rebuilt,
                *bytes,
                "the song {} does not encode back",
                path.display()
            );
            total += 1;
        }
    }
    assert!(
        total >= 136,
        "the corpus folders yielded only {total} song files, the plan wants at least 136"
    );
    println!("round-tripped {total} corpus songs");
}
