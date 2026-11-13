# USDX Song Editor

A desktop application for editing UltraStar Deluxe song files, written in Rust with a
Kirigami (Qt Quick) user interface. It opens a song in the text format that UltraStar
Deluxe sings, shows every lyric line with its notes, and lets you change pitch, length,
type and text with the mouse while listening at the exact spot.

- Application id: `io.github.ueda_ichitaka.UsdxSongEditor`
- Version: 0.1.0 (development)
- Platform: Linux, built for KDE Plasma / Arch Linux
- License: MIT

## Why

UltraStar Deluxe sings what the song file tells it. When a song's notes do not sit on the
beat, the singing feels wrong, and the usual way to fix it - editing the `.txt` file by
hand - is slow and easy to get wrong. Existing tools either rewrite the whole file (losing
comments, encodings and formatting the singer never sees) or work outside the file.

This editor takes the opposite approach: **every change touches the notes in the song file
itself and nothing else.** It reads and writes the format exactly as the format
specification defines it, keeps everything it did not edit byte-identical, and refuses to
invent content. A song that goes through the editor comes out as the same song - repaired.

Where it makes sense it complements the [USDX Song Factory](https://github.com/ueda_ichitaka/USDX-Song-Factory):
the editor can export `anchors.txt` files in the Factory's format as reference data. The
application works standalone, with no Factory, container or network needed.

## Features

- Lossless song model: encodings (UTF-8 with or without BOM, CP1252, CP1250), line
  endings, relative mode, duets, unknown note types and unknown headers all round trip
  byte-identically (verified against a real song corpus).
- Line-by-line navigation with a searchable lines pane, global line numbering across all
  voices, timing health marks for suspicious and racing lines.
- Full editing of notes and headers (pitch, length, type, text, split, merge, insert,
  delete, BPM, GAP) with unlimited undo and redo.
- Playback on the real audio files with mix, vocals and instrumental tracks, at 0.5x to
  1x speed, synced to the note sheet.
- Timing repair: anchor and stretch, block move and scale, tap-along, and a health
  analysis that names every racing line.
- Guarded workflows: unsaved changes are never lost without being asked, a file that
  changed on disk is detected on save and on window focus, and the first save of every
  session writes a `.bak` backup.
- Read-only handling for format versions it cannot write back losslessly.

See `Docs/status.md` for what is already in the current build and what is still on the
workbench.

## Install

### From source (Arch Linux / KDE Plasma)

Dependencies:

- Rust 1.100 or newer (`rustup`)
- Qt 6 (qt6-base, qt6-declarative), Kirigami 6, qqc2-desktop-style
- GStreamer (gst-plugins-base, gst-plugins-good) for playback
- extra-cmake-modules, CMake 3.28 or newer
- cxx-qt 0.10 (fetched by cargo from crates.io)

Build and install:

```sh
git clone https://github.com/ueda_ichitaka/USDX-Song-Editor.git
cd USDX-Song-Editor
cmake -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build
cmake --install build
```

This installs the `usdx-song-editor` binary, the desktop file, the AppStream metainfo and
the icon, so the application appears in Kickoff. See `Docs/building.md` for details,
development builds and tests.

## Quickstart

1. Start **USDX Song Editor** from the menu, or run `usdx-song-editor`.
2. Click **Open** in the toolbar and pick an UltraStar song file (`.txt`), or just drop a
   `.txt` file onto the window.
3. Step through the lyric lines in the lines pane on the left; the line header shows the
   time span, note count and syllable rate of the current line.
4. Edit notes on the note sheet, play the song at the current position with **Play** (or
   the Space key), and check the timing health marks before you save.
5. **Save** writes the song losslessly and keeps a one-time `.bak` backup of the file as
   it was when the session started. Ctrl+S saves, Ctrl+Z undoes.

A song without a usable `#BPM` header is refused with an explanation; format versions the
editor cannot write back losslessly open read-only.

## Documentation

| Document | Contents |
|---|---|
| `Docs/user-manual.md` | Using the application: every screen, dialog and shortcut |
| `Docs/status.md` | What works in the current build, what is planned |
| `Docs/building.md` | Building from source, tests, install, troubleshooting |
| `Docs/architecture.md` | The crates, the data flow and the testing strategy |
| `Docs/dev-script.1` | Manpage for `scripts/dev.sh`, the development entry point |
| `Plan/UI_SPEC.md` | The binding UI specification (for contributors) |

## Related projects

- [USDX Song Factory](https://github.com/ueda_ichitaka/USDX-Song-Factory) - the automated
  song repair pipeline this editor hands over to
- [UltraStar Deluxe](https://usdx.eu) - the karaoke game that sings the songs

## License

MIT, see `LICENSE`.
