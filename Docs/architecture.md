# USDX Song Editor - Architecture

How the codebase is organised, how data flows through it, and how it is tested. The
authoritative documents are `Plan/IMPLEMENTATION_PLAN.md` (sections 3, 4 and 9) and
`Plan/PROGRESS.md` (what is done); this page explains the shape in one place.

## The crates

The cargo workspace has its members under `src/`:

| Crate | Kind | Depends on | Contents |
|---|---|---|---|
| `usdx-format` | library | (Rust only) | The lossless file model: byte layer (`bytes.rs`), row classification (`rows.rs`), headers, song view, timing, serialization, validation. Knows nothing about Qt or GStreamer. |
| `usdx-edit` | library | usdx-format | Everything that changes a song: patches, undo history, note and header operations, text edits, retiming, timing health, anchors export, upgrade to format 1.1.0. |
| `usdx-audio` | library | GStreamer | The playback engine (playbin3), waveform peaks and vocal activity analysis, and the analysis cache. Knows nothing about Qt. |
| `usdx-song-editor` | binary | the three libraries, cxx-qt, Qt 6, Kirigami | The application: cxx-qt bridges, plain-Rust controllers, QML pages, C++ helpers, the main function. |

The layering rule is strict: `usdx-format` and `usdx-edit` have no Qt and no GStreamer
dependency, `usdx-audio` depends on GStreamer but not Qt, and only the app crate touches Qt
and Kirigami. Most of the test suite therefore runs without a display.

## One-way data flow

```
                  QML (Kirigami pages, note delegates, dialogs)
                       |  properties, invokables, signals
          +------------+-------------------------------+
          | usdx-song-editor                           |
          |   bridge/     cxx-qt QObjects (thin)       |
          |   controller/ plain Rust state machines    |
          +-----+------------------+-------------------+
                |                  |
        usdx-edit             usdx-audio
     (commands, undo,      (playbin3 player,
      retime, health,       peaks, activity,
      export, upgrade)      cache)
                |
        usdx-format
     (lossless document,
      song view, timing,
      validation)
```

An edit always travels the same path:

1. QML calls an invokable on a bridge QObject (for example `moveNotes()`), passing ids and
   plain values - never live Qt references into the document.
2. The bridge forwards the call to its controller.
3. The controller builds a `Patch` with `usdx-edit` operations; the history applies it to
   the `usdx-format` document and records the undo step.
4. The controller computes which rows of which models changed.
5. The bridge emits `dataChanged`, row inserts or removes, or a model reset, and QML updates
   through its bindings.

Data never flows back: QML holds no logic beyond bindings, input handling and calling
invokables, and nothing mutates the document from outside the history.

## Bridge-thin, controller-fat

Inside the app crate the split is deliberate:

- `controller/` is plain Rust, unit-tested without Qt. The `EditorController` owns the
  loaded song, the current line and selection; `shell.rs` holds the window title rules, the
  toolbar and menu enablement, the audio state and the guarded transition state machine
  (save changes? / changed on disk / stop with reason); `save.rs` is the save service with
  fingerprint check, backup and atomic write; `settings.rs` reads and writes the settings
  TOML.
- `bridge/` contains the cxx-qt QObjects. Each one only converts types (QString, QUrl,
  QVariant) and forwards calls to a controller, then mirrors controller state into
  Q_PROPERTYs and QAbstractListModel models. If you can unit-test a rule without Qt, it
  belongs in `controller/`.

Background work (waveform decoding) runs on `std::thread`; results come back to the Qt
thread through `cxx_qt::Threading::qt_thread().queue(...)`.

## Losslessness

The core invariant of the whole application lives in `usdx-format`:
`encode(decode(bytes)) == bytes` for every file the reader accepts. Everything above it is
built so that an edit changes exactly the rows it names: unknown headers, unknown note
types, empty lines, trailing bytes after `E`, original encodings and per-row line endings
survive every save. The corpus test proves it against 136 real songs; handwritten fixtures
cover the cases the corpus does not contain (BOM, CP1250, relative mode, duets, invalid
rows).

## Settings

Settings are a TOML file at
`~/.config/io.github.ueda_ichitaka.UsdxSongEditor/settings.toml`, read at start and written
atomically on change - not QSettings, so the file is inspectable and versionable. Recent
songs and the window size live in the same file.

## Testing strategy

| Level | What it covers | Tool |
|---|---|---|
| Unit | every module of `usdx-format`, `usdx-edit`, the controllers | `#[test]` |
| Property | patches, serializer, retiming invariants, text round trip | proptest |
| Snapshot | serializer output, anchors.txt output, upgrade output, validation lists | insta |
| Integration | audio: player and analysis on generated media | cargo test, needs GStreamer |
| App smoke | `usdx-song-editor --self-test` offscreen: QML loads, window created, song opens through the bridge | integration test |
| Corpus | byte-identical round trip over a real song library (136 songs) | opt-in, `USDX_CORPUS=/path` |
| Factory compatibility | exported anchors.txt parsed by the Factory's own code | opt-in, `USDX_FACTORY_DIR=/path` |
| Manual | checklist per UI task | human |

Details:

- **Unit tests** live in each crate's `tests/` directories and in-module; they are the bulk
  of the suite and run in every `scripts/dev.sh check`.
- **Property tests** drive random sequences of operations and assert invariants
  (undo round trips, patch commutation, byte-identical serialization).
- **Snapshot tests** pin the exact text of outputs that users or the Factory consume.
- **Audio tests** generate their own media (a 5 s tone with silence, as Opus and MP3) with
  GStreamer pipelines into temporary directories, so no audio fixtures are needed in git.
- **The app smoke test** runs with `QT_QPA_PLATFORM=offscreen`; it redirects its
  configuration folder to a temporary directory first, so it never touches user settings.
- **Opt-in tests** (`#[ignore]` plus an environment variable) keep `check` fast and
  independent of external folders; `scripts/dev.sh corpus` runs the corpus round trip.
- **The G-08 drawer probe** is a permanent regression test: it loads the Kirigami drawer
  offscreen and proves the shell's menu design works from our own toolbar button.

The full plan section 9 defines which real song folders may be used (read-only, copied into
temporary directories for any write test) and how audio sampling is capped.
