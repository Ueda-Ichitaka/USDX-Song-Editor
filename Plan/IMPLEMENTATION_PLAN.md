<!--
About me: the implementation plan for the USDX Song Editor. It is the single source of
truth that humans and coding agents follow to build the application task by task: the
decisions taken, the working rules, the repository layout, the data model, the algorithms,
and every task with its tests and its definition of done. The binding UI specification is
Plan/UI_SPEC.md with reference images in Plan/mockups/; the interactive mock page is
Plan/usdx-song-editor-plan.html; progress is tracked in Plan/PROGRESS.md.
-->

# USDX Song Editor - Implementation Plan

Version 1 of this plan, 12 November 2026.

The USDX Song Editor is a standalone desktop application for Arch Linux / KDE Plasma, written
in Rust with a Kirigami (Qt Quick) user interface. It opens an UltraStar Deluxe song file
(`.txt`), lets the user step through the lyric lines, change notes with the mouse and the
keyboard (timing, length, pitch, type, text), play the song at the current position, repair
lines whose timing is wrong (including "racing" sections where the USDX Song Factory lost its
anchor), and save the song losslessly. On top of that it exports `anchors.txt` files in the
Factory's format as reference data for developing the Factory.

Companion documents:

- `Plan/UI_SPEC.md`: the binding UI specification (every screen, state, dialog, size,
  colour and text), with reference images in `Plan/mockups/<screen>-light.png` and
  `-dark.png`. All UI work follows it.
- `Plan/usdx-song-editor-plan.html`: the interactive mock page and the usage workflows (open
  it in a browser; `#m-editor`, `#m-start` etc. select a screen).
  `Plan/render_mockups.sh` renders it into `Plan/mockups/`.
- `Plan/PROGRESS.md`: task status.

Precedence: for behaviour, data and algorithms this document wins; for anything visible,
`Plan/UI_SPEC.md` wins; the HTML page is illustration.

---

## Contents

0. How agents work with this plan
1. Decisions
2. Environment and toolchain
3. Repository layout
4. Architecture
5. The `usdx-format` crate: data model and file rules
6. The `usdx-edit` crate: commands, undo and algorithms
7. The `usdx-audio` crate: playback and analysis
8. The app crate: bridges, QML and UI behaviour
9. Testing strategy
10. Tasks by phase
11. Gates: questions that must go to the user
12. Reference material

---

## 0. How agents work with this plan

### 0.1 Rules (from the user's global instructions; they are not optional)

1. **TDD, always.** Red, green, refactor. Write the test first, run it, see it fail for the
   expected reason. Write the smallest code that makes it pass. Run it, see it pass. Clean up,
   run all tests again. Then the next small step.
2. **Ask on options.** When a task leaves an implementation choice open that this plan does not
   decide, stop and ask the user. Do not pick one silently. Section 11 lists known gates.
3. **No guessing.** Verify every API, signature, property name and file format against the
   pinned version's documentation or source (docs.rs, the crate source in `~/.cargo/registry`,
   `gst-inspect-1.0`, the Qt/Kirigami `.qmltypes` files under `/usr/lib/qt6/qml/`). When
   something sounds plausible, check it.
4. **No git.** Never run `git add`, `git commit`, `git push`, `git config --global`, and do not
   prepare commits. The human does all git work. Reading `git status` / `git diff` is fine.
5. **Stay inside the repository.** Read only from the repository root downwards. The only
   other paths that may be read are listed in section 12 (read-only reference material).
6. **About-me header in every source file.** Rust: a `//!` module doc comment at the top that
   says what the file does and why it exists. QML: a `//` comment block at the top. CMake,
   shell, TOML: a `#` comment block. When a file changes so that its header is no longer true,
   update the header in the same task.
7. **Comments describe the present.** No history in comments ("used to", "no longer",
   "changed from"). History belongs in git.
8. **New dependency: check the latest version first** on crates.io (API:
   `https://crates.io/api/v1/crates/<name>`), then add it with that exact version. Section 2.4
   has the versions checked on 12 November 2026; check them again on the day you add them.
9. **No emojis** in code, docs or markdown. Only U+2705 and U+274C are allowed, and only in
   tables and overviews. Never use the en dash (U+2013); use the hyphen-minus `-`.
10. **Resources.** Check memory and CPU at the start of every job
    (`free -h`, `nproc`). You are not the only process on the machine. Build and test only
    through `scripts/dev.sh`, which caps parallelism and runs the job in a memory-limited
    systemd scope so an out-of-memory situation kills the build, not the desktop.

### 0.2 Workflow for one task

1. Read the task in section 10, the sections it references, and `Plan/PROGRESS.md`.
2. Check that all tasks listed under "Needs" are marked done in `Plan/PROGRESS.md`.
3. Check resources (`free -h`, `nproc`).
4. RED: write the tests listed under "Tests first". Run them with `scripts/dev.sh test <crate>`.
   They must fail, and fail for the reason you expect (not a compile error in an unrelated
   place).
5. GREEN: implement the smallest change that passes.
6. REFACTOR: remove duplication, improve names, keep headers and comments true. Run
   `scripts/dev.sh check` (fmt, clippy with `-D warnings`, all tests).
7. UI tasks only: run the acceptance procedure of `Plan/UI_SPEC.md` section 8.2 (render the
   task's screens in light and dark with `scripts/ui-shots.sh`, compare them with
   `Plan/mockups/`, record the result and every deviation with its reason).
8. Update `Plan/PROGRESS.md`: mark the task, note anything a later task must know.
9. Report to the user what was done, which tests were added, and anything left open. Do not
   commit.

### 0.3 Definition of done (every task)

- All tests listed under "Tests first" exist and pass; no test was weakened to pass.
- `scripts/dev.sh check` passes (rustfmt, clippy `-D warnings`, `cargo test --workspace`).
- Every new or changed file has a correct about-me header.
- No `unwrap()`/`expect()` on data that comes from files, the user, GStreamer or Qt; those
  return errors. `expect()` is allowed only for true invariants, with a message saying why.
- No `unsafe` outside the generated cxx-qt bridges and the `paint` override required by
  `QQuickPaintedItem`.
- UI tasks: the screens match `Plan/UI_SPEC.md` and `Plan/mockups/` in both themes
  (UI_SPEC 8.2); no literal colours or pixel sizes outside `Palette.qml` and `Metrics.qml`.
- `Plan/PROGRESS.md` updated.

---

## 1. Decisions

All decided by the user on 12 November 2026.

| ID | Topic | Decision |
|---|---|---|
| D-01 | Rust/QML binding | cxx-qt 0.10 (KDAB), as in KDE's "Kirigami with Rust" guide |
| D-02 | Audio | GStreamer through gstreamer-rs 0.25 (`playbin3`, `scaletempo`, `appsink`) |
| D-03 | Timing repair tools | All four: anchor + stretch, tap-along, block move/scale, anchors.txt export |
| D-04 | Saving | Lossless round trip; an explicit "Upgrade to format 1.1.0" action; backup before the first save |
| D-05 | Product shape | Standalone. All repairs change the notes in the song; Save writes the file. The anchors export is an extra for Factory development, never the way changes are applied |
| D-06 | Application ID | `io.github.ueda_ichitaka.UsdxSongEditor`; display name "USDX Song Editor"; binary `usdx-song-editor` |
| D-07 | Backup | `<song>.txt.bak` beside the song, written on the first save of each session (overwrites the previous session's backup) |
| D-08 | Duets | Load, display (one voice at a time) and save losslessly in the first release; editing more than voice P1 comes in phase 9 |
| D-09 | Note sheet rendering | Notes are QML delegates (DragHandler, hit testing, cursors); grid, ruler and waveform are one Rust `QQuickPaintedItem` behind them |
| D-10 | UI language | English only, but every user-visible QML string goes through `qsTr()` from the first line of QML, so translations can be added later |
| D-11 | Upgrade and `#VERSION` | Upgrade sets `#VERSION:1.1.0` only when the file is unversioned or `1.0.0`. A file that already says `1.1.0` or a higher `1.x.y` (UltraSinger writes `1.2.0`) keeps its version |
| D-12 | anchors.txt content | Every line whose start time changed in this editing session (compared with the file as loaded), plus the pinned anchors and the end pin |
| D-13 | Code location | Cargo workspace; the crates live under the existing `src/` folder |
| D-14 | UI specification | `Plan/UI_SPEC.md` with the images in `Plan/mockups/` is binding for everything visible, so that any agent produces the same UI; every UI task is accepted by screenshot comparison in both themes |

Ruled out: symphonia (version 0.6.1 has no Opus decoder; most of the library is `.opus`),
qmetaobject-rs (last release 2023), Qt Multimedia (less control over position, rate and
samples).

---

## 2. Environment and toolchain

### 2.1 Machine (checked 12 November 2026)

- 16 CPU threads, 30 GiB RAM. Other workloads run on this machine.
- Arch Linux (CachyOS kernel), KDE Plasma.
- Rust 1.100.0 stable through rustup in `~/.cargo/bin`. It is on the user's fish `PATH`, but
  not on the zsh `PATH` that agent shells use: call `~/.cargo/bin/cargo` or let
  `scripts/dev.sh` set `PATH`.

### 2.2 System packages present

| Package | Version |
|---|---|
| qt6-base, qt6-declarative | 6.12.0 |
| kirigami, qqc2-desktop-style, extra-cmake-modules, kcoreaddons, ki18n, kconfig | 6.31.0 |
| kirigami-addons | 1.16.1 |
| breeze | 6.8.2 |
| gstreamer, gst-plugins-base, gst-plugins-good | 1.28.9 |
| cmake | 4.4.5 |
| ffmpeg | 9.0.2 |

GStreamer elements verified with `gst-inspect-1.0`: `playbin3` (properties `uri`,
`audio-filter`, `audio-sink`, `volume`, `instant-uri`), `scaletempo`, `opusdec`,
`mpg123audiodec`, `uridecodebin`, `audioconvert`, `audioresample`, `appsink`, and for test
fixtures `audiotestsrc`, `opusenc`, `oggmux`, `lamemp3enc`, `id3v2mux`, `wavenc`.

Qt/Kirigami types verified in `/usr/lib/qt6/qml`: `FrameAnimation` (QtQuick 6.4+),
Kirigami `ApplicationWindow`, `GlobalDrawer`, `ContextDrawer`, `Page`, `ScrollablePage`,
`FormLayout`, `InlineMessage`, `Dialog`, `PromptDialog`, `Chip`, `ActionToolBar`,
`SearchField`, `Separator`, `Heading`; Kirigami Addons `formcard`.

### 2.3 Facts verified in cxx-qt 0.10.0 sources

- `cxx_qt_build::CxxQtBuilder::new_qml_module(QmlModule::new(uri).qml_files([...]))`
  `.files([...]).build()` builds the QML module; `QmlModule` also has `qml_file`, `version`,
  `depend(s)`.
- `#[qobject] #[base = QAbstractListModel]` with `#[cxx_override]` on `data`, `row_count`
  (`rowCount`), `role_names` (`roleNames`), and `#[inherit]` for `beginInsertRows`,
  `endInsertRows`, `beginRemoveRows`, `endRemoveRows`, `beginResetModel`, `endResetModel`,
  the `dataChanged` signal (example `examples/qml_features/rust/src/custom_base_class.rs`).
- `#[base = QQuickPaintedItem]` with `#[cxx_override] unsafe fn paint(self: Pin<&mut Self>,
  painter: *mut QPainter)` and `#[inherit] fn update` / `fn size` (example
  `custom_parent_class.rs`). `QQuickPaintedItem` is declared in an `extern "C++"` block with
  `include!(<QtQuick/QQuickPaintedItem>)`.
- `cxx_qt_lib::QPainter` binds `draw_line`, `draw_linef`, `draw_lines`, `draw_polyline`,
  `draw_rect_f`, `draw_rounded_rect`, `draw_text`, `fill_rect`, `set_pen`, `set_font`,
  `set_render_hint`, `set_clip_rect`, `save`, `restore`, `translate`, `set_opacity` among
  others.
- `impl cxx_qt::Threading for T {}` gives `qt_thread()` to queue closures onto the Qt thread.
- `cxx_qt_lib::QQmlEngine::add_image_provider` exists (since 0.9).
- `cxx-qt-lib` 0.10 has **no** `QSettings`, `QTranslator` or `QFileSystemWatcher` bindings.
  Settings are therefore a TOML file written from Rust (section 8.8).
- KDE's guide (written for cxx-qt 0.8.1) uses `cxx_qt_lib_extras::QApplication`,
  `QGuiApplication::set_desktop_file_name`, `QQuickStyle::set_style("org.kde.desktop")`
  unless `QT_QUICK_CONTROLS_STYLE` is set, and `QQmlApplicationEngine::load(qrc url)`.
  Task P0-03 confirms the same calls compile on 0.10.

### 2.4 Crate versions (crates.io, 12 November 2026; check again when adding)

| Crate | Version | Used in |
|---|---|---|
| cxx | 1.0.205 | app |
| cxx-qt, cxx-qt-lib (feature `qt_full`), cxx-qt-lib-extras | 0.10.0 | app |
| cxx-qt-build (build-dependency) | 0.10.0 | app |
| gstreamer | 0.25.4 | usdx-audio |
| gstreamer-app | 0.25.2 | usdx-audio |
| encoding_rs | 0.8.44 | usdx-format |
| chardetng | 1.0.0 | usdx-format |
| thiserror | 2.0.21 | all library crates |
| serde | 1.0.229 | app (settings) |
| toml | 1.2.1 | app (settings) |
| dirs | 7.0.0 | app (config and cache directories) |
| similar | 3.2.1 | usdx-edit (upgrade diff) |
| log / env_logger | 0.4.35 / 0.11.11 | all / app |
| proptest (dev) | 1.12.0 | tests |
| insta (dev) | 1.49.0 | tests |
| tempfile (dev) | 3.27.0 | tests |

Do not add crates beyond this list without asking the user.

### 2.5 `scripts/dev.sh` (task P0-02)

Subcommands: `check` (fmt check, clippy `-D warnings`, all tests), `test [crate] [filter]`,
`build [--release]`, `run [-- args]`, `corpus` (round-trip test over `USDX_CORPUS`, by
default the test song folders of section 9.5).

Resource guard, run before every subcommand:

1. Read `MemAvailable` from `/proc/meminfo` (path overridable by `DEV_MEMINFO` for tests).
2. Refuse to start below 4 GiB available, with a message saying how much is free.
3. `CARGO_BUILD_JOBS = max(1, min(nproc / 2, MemAvailable_GiB / 2))`.
4. Run cargo inside `systemd-run --user --scope -q -p MemoryMax=<70 % of MemAvailable>`
   (verified to work on this machine). Exit code 137, or an `oom-kill` entry for the scope in
   `journalctl --user`, is reported as "killed: out of memory" and not as a test failure.
5. Prepend `~/.cargo/bin` to `PATH`.

---

## 3. Repository layout

```
USDX-Song-Editior/
  Cargo.toml                     workspace: members = ["src/*"], shared [workspace.dependencies]
  CMakeLists.txt                 installs binary, .desktop, metainfo, icon (ECM)
  scripts/
    dev.sh                       guarded build/test entry point (section 2.5)
    ui-shots.sh                  renders the app's demo states to PNG (UI_SPEC 8.1)
    tests/dev_guard_test.sh      tests for the guard functions
  src/
    usdx-format/                 lib: lossless file model, parse, serialize, timing, validation
      src/lib.rs, bytes.rs, rows.rs, header.rs, song.rs, timing.rs, serialize.rs, validate.rs
      tests/fixtures/*.txt       handwritten fixture songs (section 9.3)
      tests/roundtrip.rs, tests/corpus.rs
    usdx-edit/                   lib: patches, undo, note ops, text, retiming, health, export, upgrade
      src/lib.rs, patch.rs, history.rs, ops.rs, header_ops.rs, text.rs, collisions.rs,
          retime.rs, block.rs, tap.rs, health.rs, anchors_export.rs, upgrade.rs, changes.rs
    usdx-audio/                  lib: GStreamer player, peaks and vocal activity, cache
      src/lib.rs, player.rs, analysis.rs, cache.rs, testmedia.rs (test helper, cfg(test))
    usdx-song-editor/            bin: cxx-qt bridges, controllers, QML
      Cargo.toml, build.rs
      src/main.rs
      src/controller/            plain Rust, no Qt: editor state, selection, viewport, repair, tap
      src/bridge/                thin cxx-qt QObjects that call the controllers
      src/settings.rs, src/save.rs
      qml/Main.qml, EditorPage.qml, ... (section 8.5)
  packaging/
    io.github.ueda_ichitaka.UsdxSongEditor.desktop
    io.github.ueda_ichitaka.UsdxSongEditor.metainfo.xml
    io.github.ueda_ichitaka.UsdxSongEditor.svg
    PKGBUILD
  Docs/
    user-manual.md               written in phase 8
  Plan/
    IMPLEMENTATION_PLAN.md       this file
    UI_SPEC.md                   binding UI specification
    PROGRESS.md                  task checklist
    usdx-song-editor-plan.html   interactive mocks and workflows
    render_mockups.sh            renders the mock page into mockups/
    mockups/                     reference images, <screen>-light.png and <screen>-dark.png
```

Workspace `Cargo.toml`: `edition = "2024"`, `rust-version = "1.100"`,
`resolver = "3"` (verify that edition 2024 workspaces need `resolver = "3"` stated
explicitly in the root manifest; cargo prints a warning if not).

---

## 4. Architecture

```
                  QML (Kirigami pages, note delegates, dialogs)
                       |  properties, invokables, signals
          +------------+-------------------------------+
          | usdx-song-editor                           |
          |   bridge/  cxx-qt QObjects (thin)          |
          |   controller/  plain Rust state machines   |
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

Rules:

- `usdx-format` and `usdx-edit` have no Qt and no GStreamer dependency. Most tests live here.
- `usdx-audio` depends on GStreamer only, not on Qt.
- In the app crate, all logic sits in `controller/` (plain Rust, unit-tested without Qt). The
  `bridge/` QObjects only convert types and forward calls. QML holds no logic beyond bindings,
  input handling and calling invokables.
- Data flows one way: QML calls an invokable, the controller builds a `Patch` with
  `usdx-edit`, the history applies it to the `usdx-format` document, the controller computes
  which rows of which models changed, and the bridge emits `dataChanged`, row inserts or
  removes, or a model reset.
- Background work (waveform decoding) runs on `std::thread`; results come back to the Qt
  thread through `cxx_qt::Threading::qt_thread().queue(...)`.

---

## 5. The `usdx-format` crate

### 5.1 File rules from the spec

Sources: `UltraStar-Deluxe/format` repository, files "The UltraStar File Format (v1).md",
"(Unversioned).md", "(v2).md" (draft), and usdx.eu/format (lists 1.1.0 as the current
published version).

| Topic | Rule | Behaviour |
|---|---|---|
| Encoding | UTF-8 MUST, no BOM MUST NOT be added; readers MAY ignore a BOM | Detect and keep. Upgrade removes the BOM |
| Legacy encoding | Unversioned files may carry `#ENCODING` (UTF-8, CP1252, CP1250) | Respect it; keep on save; upgrade converts to UTF-8 |
| Line endings | CR, LF or CRLF; EOF ends a line too | Keep each row's own terminator |
| Empty lines | Ignored everywhere | Kept verbatim as rows |
| Header | `#KEY:VALUE`, key case-insensitive, whitespace around key and value ignored, empty value = absent | Parse; keep the raw row |
| Unknown headers | MUST be ignored by readers | Kept verbatim |
| Version | Same major version can be processed; reject syntactically invalid | Accept 1.x.y and unversioned; 2.x opens read-only with an error |
| BPM | `1*DIGIT [(period / comma) 1*DIGIT]` (v1), implicitly quadrupled | Accept comma and period |
| GAP | ms from audio start to beat 0, decimal allowed in v1 | `f64` ms |
| #START / #END | START in seconds, END in milliseconds (v1) | Shown in properties; no effect on note placement |
| Note | `type WSP start WSP duration WSP pitch WSP text` | See 5.3 |
| Note types | `:` `*` `R` `G` `F`; unknown types MAY be treated as F, MUST NOT get other semantics | Unknown types are kept byte-exact, shown as freestyle with a warning |
| Start beat | `1*DIGIT`, no minus | Never write a negative beat; retiming lowers `#GAP` instead |
| Pitch | Half steps relative to C4, may be negative; ignored for R, G, F | Kept; greyed out in the UI for R, G, F |
| Note text | Spaces before or after are allowed and equivalent | Stored exactly; moving a space between neighbouring notes is not a timing change |
| Phrase end | `- beat` (v1); `- beat rel-offset` in relative mode; not twice in a row; not inside a note | Validated; retiming places breaks again |
| Voices | `P1` to `P9` (unversioned: P1, P2), `#P1`... headers required when used; no leading Pn means P1 | Parsed into voices |
| End | `E` on its own line; everything after it is ignored | Bytes after `E` kept verbatim |
| Relative mode | `#RELATIVE:yes` (unversioned only) | Read support in phase 1; editing behind gate G-01 |
| File references | Relative to the song file; absolute paths SHOULD NOT be used; Linux is case-sensitive | Resolve relative; case-insensitive fallback with a warning; refuse absolute paths |

### 5.2 Byte layer (`bytes.rs`)

```rust
pub enum TextEncoding { Utf8, Windows1252, Windows1250 }
pub enum EncodingSource { Valid, Header, Guessed }   // how the encoding was found
pub enum Terminator { Lf, CrLf, Cr, None }           // None only on the last row
pub struct RawLine { pub text: String, pub terminator: Terminator }
pub struct DecodedFile {
    pub bom: bool,
    pub encoding: TextEncoding,
    pub encoding_source: EncodingSource,
    pub lines: Vec<RawLine>,
}
pub fn decode(bytes: &[u8]) -> Result<DecodedFile, FormatError>;
pub fn encode(file: &DecodedFile) -> Result<Vec<u8>, FormatError>; // exact inverse of decode
```

Detection order: a UTF-8 BOM means UTF-8. Otherwise, valid UTF-8 means UTF-8. Otherwise, an
ASCII-readable `#ENCODING:` header names it. Otherwise, chardetng chooses between
windows-1252 and windows-1250 (`EncodingSource::Guessed`, shown as a warning). The property
`encode(decode(b)) == b` must hold for every fixture; test the windows-1252 bytes 0x80 to 0x9F
explicitly (verify how encoding_rs maps the unassigned positions 0x81, 0x8D, 0x8F, 0x90, 0x9D
and make the round trip exact or fail loudly).

### 5.3 Rows (`rows.rs`)

```rust
pub struct RowId(u32);                      // stable for the life of a loaded document
pub enum NoteType { Normal, Golden, Freestyle, Rap, RapGolden, Other(char) }
pub struct NoteRow { pub kind: NoteType, pub start: i64, pub length: i64, pub pitch: i64, pub text: String }
pub enum RowKind {
    Header { key: String, value: String },   // key upper-cased for lookup; raw kept in Row
    Note(NoteRow),
    PhraseEnd { beat: Option<i64>, rel_offset: Option<i64> },
    Voice(u8),
    End,
    Empty,
    Invalid { reason: String },              // kept verbatim, reported by validation
}
pub struct Row { pub id: RowId, pub raw: RawLine, pub kind: RowKind, pub dirty: bool }
pub struct Document { pub file: DecodedFileMeta, pub rows: Vec<Row>, pub trailing: Vec<RawLine> }
```

Classification per line, in this order:

1. Whitespace only: `Empty`.
2. Before the first body row, a line starting with `#`: `Header`. Split at the first `:`;
   trim key and value. A `#` line without `:` or after the body started: `Invalid`.
3. `E` alone (after trimming whitespace): `End`. All following lines go to `trailing`.
4. `P` followed by a digit (`P1`); accept `P 1` with a warning: `Voice`.
5. `-` then whitespace then digits (and a second number in relative mode): `PhraseEnd`.
6. A visible ASCII character other than space and `#`, then whitespace, then three integer
   fields separated by one or more whitespace characters, then optionally exactly one
   whitespace character followed by the text (the rest of the line, verbatim, may start with
   spaces): `Note`. Whitespace means Unicode `White_Space` except CR and LF (tab is common in
   real files). A negative start or duration parses with a warning.
7. Anything else: `Invalid`.

Numeric fields are `i64` so invalid values (negative starts) can be held and reported; the
editor never writes a negative start or a length below 1.

### 5.4 Song view (`song.rs`)

A derived, read-only index rebuilt after each change (songs have about 1000 rows, so a
rebuild is cheap; measure in P1-05 and keep it below 1 ms for 3000 rows in release mode).

```rust
pub struct Song {
    pub headers: Headers,                    // case-insensitive lookup, first occurrence wins
    pub timing: Option<Timing>,              // None without a valid #BPM
    pub version: Version,                    // Unversioned | V(major, minor, patch) | Invalid(String)
    pub relative: bool,
    pub voices: Vec<Voice>,
}
pub struct Voice { pub number: u8, pub lines: Vec<Line> }
pub struct Line { pub notes: Vec<RowId>, pub end: Option<RowId>, pub index: usize }
impl Song {
    pub fn from_document(doc: &Document) -> Song;
    pub fn note(&self, doc: &Document, id: RowId) -> Option<AbsNote>; // absolute beats
}
```

A line ends at a `PhraseEnd`, at a `Voice` change, or at `End`, as in the Factory's
`note_types.py`. Line indices count from 0 internally and are shown from 1 in the UI.

### 5.5 Timing (`timing.rs`)

```rust
pub struct Timing { pub bpm: f64, pub gap_ms: f64 }
impl Timing {
    pub fn ms_per_beat(&self) -> f64 { 60_000.0 / (self.bpm * 4.0) }
    pub fn beat_to_ms(&self, beat: f64) -> f64 { self.gap_ms + beat * self.ms_per_beat() }
    pub fn ms_to_beat(&self, ms: f64) -> f64 { (ms - self.gap_ms) / self.ms_per_beat() }
}
```

Test values from the sample song "Realize The Real Lies" (`#BPM:369.14`, `#GAP:14937`):
beat 0 is at 14 937 ms, beat 161 at 21 479.1 ms (14937 + 161 * 40.6339...), one beat is
40.634 ms.

### 5.6 Serializer (`serialize.rs`)

- A row with `dirty == false` is written as its `raw.text` plus its own terminator.
- A dirty row is formatted canonically:
  - note: `"{type} {start} {length} {pitch} {text}"` with single ASCII spaces; the text is
    written exactly as stored (a leading space in the text produces two spaces in a row);
  - phrase end: `"- {beat}"` (relative mode: gate G-01);
  - header: the original raw prefix up to and including the first `:` is kept and only the
    value is replaced, so `#Title:` keeps its spelling. A new header is `"#{KEY}:{value}"`
    inserted after the last header row.
- A new row takes the document's dominant terminator (the most common one; LF when tied).
- BOM, encoding and `trailing` are written back unchanged.
- `serialize(parse(bytes)) == bytes` for every input that `decode` accepts.

### 5.7 Validation (`validate.rs`)

`pub fn validate(doc: &Document, song: &Song, folder: Option<&Path>) -> Vec<Issue>`, where
`Issue { severity: Error | Warning | Info, kind: IssueKind, row: Option<RowId>, message: String }`.
The kinds follow the karaoke-dashboard's `app/format_check.py` so both tools name problems the
same way:

Missing `#TITLE`, `#ARTIST`, `#BPM`; no audio reference (`#MP3` or `#AUDIO`); missing
`#VERSION` (Warning); invalid `#VERSION` syntax (Error); major version other than 1 (Error,
document becomes read-only); unpublished version such as `1.2.0` (Info: "not a published
format version; read as version 1"); malformed numeric header; BOM present (Warning); not
UTF-8 (Warning); encoding guessed (Warning); referenced file missing; referenced file found
only with different letter case (Warning); absolute path in a file reference (Error); `Pn`
used without `#Pn` header (Warning); `Invalid` rows; unknown note type (Warning); negative
start or length below 1 (Error); notes out of order within a voice; overlapping notes; phrase
end inside a note; two phrase ends in a row; phrase end before the first or after the last
note; note with empty text (Warning).

---

## 6. The `usdx-edit` crate

### 6.1 Patches (`patch.rs`)

Every edit is a `Patch`: a list of row operations with enough data to invert them exactly.

```rust
pub enum RowOp {
    Replace { id: RowId, before: Row, after: Row },
    Insert  { after: Option<RowId>, row: Row },     // None = at the start of the body
    Remove  { id: RowId, row: Row, after: Option<RowId> },
}
pub struct Patch { pub label: String, pub ops: Vec<RowOp> }
impl Patch {
    pub fn apply(&self, doc: &mut Document) -> Result<(), EditError>;
    pub fn inverse(&self) -> Patch;
}
```

Invariant (property test): for any document and any valid patch,
`apply(p); apply(p.inverse())` gives a document equal to the original, including `dirty`
flags and raw text.

### 6.2 History (`history.rs`)

```rust
pub struct History { /* undo stack, redo stack, saved position */ }
impl History {
    pub fn commit(&mut self, doc: &mut Document, patch: Patch) -> Result<(), EditError>;
    pub fn undo(&mut self, doc: &mut Document) -> Option<String>;   // label of the undone step
    pub fn redo(&mut self, doc: &mut Document) -> Option<String>;
    pub fn mark_saved(&mut self);
    pub fn is_dirty(&self) -> bool;          // current position differs from the saved one
    pub fn undo_label(&self) -> Option<&str>;
    pub fn redo_label(&self) -> Option<&str>;
}
```

A drag commits once, on release. Previews during a drag or in repair mode are computed but
not committed (`Patch` applied to a clone, or shown as ghost positions).

### 6.3 Note and line operations (`ops.rs`)

Each function takes `&Document` and `&Song` and returns `Result<Patch, EditError>`.

| Function | Behaviour |
|---|---|
| `move_notes(ids, d_beat, d_pitch)` | Moves notes; fails with `BeforeSongStart` if a start would go below 0 |
| `move_notes_and_following(ids, d_beat)` | Also moves every later note and phrase end of the voice (Yass "shift remainder") |
| `resize_start(id, d_beat)`, `resize_end(id, d_beat)` | Length never below 1 |
| `set_pitch(ids, pitch)`, `transpose(ids, semitones)` | |
| `set_type(ids, NoteType)` | Rewrites only the type character of the raw row (row stays clean except that byte; mark dirty and format canonically) |
| `toggle_freestyle(ids)` | All F become `:`; otherwise all become F (Factory behaviour) |
| `set_text(id, text)` | |
| `insert_note(after: RowId or line, start, length, pitch, text)` | Default text `"~"` |
| `delete_notes(ids)` | Removes a phrase end left with no notes before it |
| `split_note(id, at_beat)` | Left part keeps the text, right part gets `"~"`; total length kept |
| `merge_with_next(id)` | Text joined (a `"~"` of the right note is dropped), length to the end of the right note; not across a phrase end |
| `move_phrase_end(id, beat)` | Only between the end of the previous note and the start of the next |
| `toggle_phrase_end_before(note_id)` | Inserts `- <end beat of the previous note>` (the convention in UltraSinger files, for example `- 56` after `: 45 11 9 ror`) or removes the existing one |

### 6.4 Header operations (`header_ops.rs`)

`set_header(key, value)`, `remove_header(key)`, `set_gap_ms(ms)`,
`set_bpm(bpm, keep_times: bool)`. With `keep_times`, every beat is rescaled
(`beat' = round(beat * new_bpm / old_bpm)`, lengths at least 1) so that the notes stay at the
same audio time.

### 6.5 Line text (`text.rs`)

Syntax of the line text field: a space separates words; `|` splits a word into syllables;
`~` at the start of a syllable continues the previous one on a new note (a held note).

```rust
pub struct Syllable { pub text: String }      // text as it goes into the note, spaces included
pub fn parse_line_text(input: &str) -> Vec<Syllable>;
pub fn line_text_for_editing(notes: &[NoteRow]) -> String;  // inverse for display
pub fn plan_text_change(line: &[(RowId, NoteRow)], syllables: &[Syllable], policy: CountPolicy)
    -> Result<Patch, EditError>;
```

When the counts match, text is assigned note by note. When there are more syllables than
notes, the longest note is split in halves (rounded) until the counts match; when there are
fewer, the shortest neighbouring pairs are merged. `CountPolicy::Ask` returns
`EditError::NeedsDecision` with the proposed mapping, which the dialog shows. Round trip:
`parse_line_text(line_text_for_editing(notes))` reproduces the note texts exactly.

### 6.6 Collision resolution (`collisions.rs`)

Input: one voice's notes with new float start times and lengths in beats (from a retime).
Output: integer beats that satisfy the invariants.

1. Round starts and ends to the nearest beat; length at least 1.
2. Walk in order. If a note starts before the previous note's end, first shorten the previous
   note down to a minimum of 1 beat; if that is not enough, move the current note to the
   previous note's end.
3. Phrase ends: each break between line k and k+1 keeps its relative position inside the gap
   between the last note of line k and the first note of line k+1 (old gap fraction applied to
   the new gap), rounded, and clamped into `[end of last note of k, start of first note of k+1]`.
4. If any start is below 0, return `NeedsGapShift(beats)`; the caller lowers `#GAP` by that many
   beats in ms and shifts every note of every voice by the same number of beats, in the same
   patch.

Invariants (property tests): starts never decrease along the voice, no overlap, no phrase end
inside a note, no two phrase ends in a row, all values are integers, starts at least 0,
lengths at least 1.

### 6.7 Anchor + stretch (`retime.rs`)

```rust
pub struct Anchor { pub line: usize, pub target_ms: f64 }
pub enum Outside { Shift, Keep }               // before the first / after the last anchor
pub enum Lengths { Scale, Keep }
pub struct StretchOptions { pub before: Outside, pub after: Outside, pub lengths: Lengths, pub end_pin_ms: Option<f64> }
pub fn anchor_stretch(doc: &Document, song: &Song, voice: u8, anchors: &[Anchor], opts: &StretchOptions)
    -> Result<Patch, EditError>;
```

For two neighbouring anchors `a` and `b` (sorted by line; the times must increase, otherwise
`EditError::AnchorsOutOfOrder`), every note time `t` (in ms) between them maps to

```
t' = a.new + (t - a.old) * (b.new - a.new) / (b.old - a.old)
```

where `a.old` is the current start time of anchor line `a`. The note end maps with the same
formula when `Lengths::Scale`; with `Lengths::Keep` the length in beats stays. Before the first
anchor: shift by `first.new - first.old` (`Outside::Shift`) or leave unchanged. After the last
anchor: shift by the last offset, keep, or stretch to `end_pin_ms` (the end pin maps the end of
the last note). The result goes through the collision pass (6.6). Additional invariant: every
anchor line starts within 0.5 beat of its target.

Acceptance test (synthetic fault, the Factory's lesson 5 method): take a correct song,
compress lines 15 to 22 to a quarter of their duration (keeping texts and pitches), then
repair with anchors at the original start of line 15 and line 23. Every note start must come
back within 1 beat of the original.

### 6.8 Block move/scale (`block.rs`)

`block_transform(doc, song, voice, lines: Range<usize>, new_start_ms, new_end_ms, follow: Follow)`
with `Follow::Stay | Follow::MoveAlong`. Implemented as an anchor stretch with two anchors at
the block's first note start and last note end, `Outside::Keep` before. With `MoveAlong`,
everything after the block shifts by the end offset; with `Stay`, notes after the block keep
their place and the collision pass resolves overlaps at the boundary.

### 6.9 Tap mapping (`tap.rs`)

```rust
pub struct Tap { pub down_ms: f64, pub up_ms: Option<f64> }   // audio time, latency already removed
pub fn apply_line_taps(.., lines: Range<usize>, taps: &[Tap]) -> Result<Patch, EditError>;
pub fn apply_syllable_taps(.., notes: &[RowId], taps: &[Tap]) -> Result<Patch, EditError>;
```

Line mode: each tap is an anchor for the next line in the range (reuses 6.7). Syllable mode:
each tap sets the start of the next note; the length is `up - down` when held, otherwise
`min(old length * local factor, next tap - 1 beat)`, where the local factor is the ratio of the
tapped span to the old span of the same notes. Fewer taps than notes: only the tapped notes
change, the rest of the line is stretched between the last tap and the next unchanged note.

### 6.10 Racing detection (`health.rs`)

```rust
pub struct HealthConfig { rate_warn: f64, rate_bad: f64, neighbour_factor_warn: f64,
                          coverage_warn: f64, coverage_bad: f64, neighbourhood: usize }
pub enum Health { Ok, Suspicious, Racing }
pub struct LineHealth { pub line: usize, pub health: Health, pub syllables_per_s: f64,
                        pub neighbour_factor: f64, pub vocal_coverage: Option<f64>, pub reason: String }
pub fn analyze(doc, song, voice, activity: Option<&Activity>, cfg: &HealthConfig) -> Vec<LineHealth>;
pub fn racing_sections(health: &[LineHealth]) -> Vec<Range<usize>>;   // neighbouring Racing lines merged
```

Defaults (first guesses, tuned in task R-10 and stored in the settings): `rate_warn = 7.0`,
`rate_bad = 10.0` notes per second; `neighbour_factor_warn = 2.5` against the median of the
`neighbourhood = 6` surrounding lines; `coverage_warn = 0.40`, `coverage_bad = 0.20` (share of
note time where the vocals track is active). The worst signal decides; the reason names it.
Without a `#VOCALS` track, the coverage signal is `None`.

### 6.11 anchors.txt export (`anchors_export.rs`, `changes.rs`)

Format read by the Factory (`USDX-Song-Factory/stack/orchestrator/anchors.py` and
`outro.py:parse_time`, documented in `docs/repairs.md`):

- UTF-8 (a BOM is tolerated by the Factory; write none). Empty lines and lines starting with
  `#` are ignored.
- Anchor line: `<time> | <lyric line>`. Time is `SS[.fff]`, `M:SS[.fff]` or `H:MM:SS[.fff]`,
  seconds and minutes after a colon below 60, a dot as the decimal separator.
- Times must strictly increase. At most one `end <time>` line, after the last anchor and later
  than it.
- The Factory matches each anchor's text against the lines of `lyrics.txt`, normalised as
  lower case, every non-word non-space character replaced by a space, whitespace collapsed.
  Matching runs from the end: the last anchor binds to the last matching lyrics line, each
  earlier anchor to the last match before the one bound after it.

Content (decision D-12): every line of voice P1 whose first note's start time differs from the
document as loaded (`changes.rs` compares the current song with a snapshot taken at load, by
`RowId`), plus the anchors pinned in this session, plus the end pin if one is set. Deduplicate
by line; sort by time. Time = start of the line's first note in ms from the start of the audio
(`Timing::beat_to_ms`), written as `M:SS.ss` (two decimals). Text = the line's syllables joined,
`~` removed, whitespace collapsed.

Output file: chosen in a save dialog, default `<song folder>/anchors.txt`; never overwrite
without confirmation. A header comment names the song, the editor version and the date.

Checks before writing, shown in the export dialog:

1. If `lyrics.txt` exists in the song folder, run the Factory's matching (ported 1:1 to Rust,
   with tests copied from the behaviour of `match_anchor_units`) and list anchors whose text is
   not found or that bind to a different occurrence than the song line they came from.
2. Without `lyrics.txt`, warn when an exported line's normalised text also appears on a later
   line of the song (the Factory would bind it to the later one).

### 6.12 Upgrade to format 1.1.0 (`upgrade.rs`)

`pub fn plan_upgrade(doc: &Document, song: &Song) -> UpgradePlan` where `UpgradePlan` holds the
new bytes and a list of human-readable changes; `similar` provides the line diff the dialog
shows. Rules:

- `#VERSION`: set `1.1.0` when absent or `1.0.0`; leave `1.1.0` and any higher `1.x.y` (D-11).
- Keep `#MP3` (required in v1). If `#AUDIO` is missing, add it with the `#MP3` value.
- Remove `#ENCODING`; convert the text to UTF-8; remove the BOM; LF line endings; make sure the
  file ends with an `E` row followed by a newline.
- `#RELATIVE:no` is removed. `#RELATIVE:yes` is converted to absolute beats first (gate G-01).
- `#BPM`/`#GAP` with a decimal comma stay valid in v1 and are not changed.
- The upgrade is one undoable patch plus file-level changes (encoding, BOM, endings) that apply
  on the next save; the properties page shows them as pending.

---

## 7. The `usdx-audio` crate

### 7.1 Player (`player.rs`)

```rust
pub enum Track { Mix, Vocals, Instrumental }
pub enum PlayerEvent { StateChanged(PlayState), EndOfStream, RangeDone, Error(String), DurationKnown(f64) }
pub struct Player { /* playbin3 */ }
impl Player {
    pub fn new() -> Result<Player, AudioError>;       // gst::init once, playbin3, audio-filter = scaletempo
    pub fn load(&mut self, path: &Path) -> Result<(), AudioError>;
    pub fn play(&mut self); pub fn pause(&mut self);
    pub fn seek_ms(&mut self, ms: f64) -> Result<(), AudioError>;          // FLUSH | ACCURATE
    pub fn play_range(&mut self, start_ms: f64, end_ms: f64) -> Result<(), AudioError>; // seek with stop
    pub fn set_rate(&mut self, rate: f64) -> Result<(), AudioError>;       // 0.5, 0.75, 1.0
    pub fn position_ms(&self) -> Option<f64>;
    pub fn duration_ms(&self) -> Option<f64>;
    pub fn poll(&mut self) -> Vec<PlayerEvent>;    // drains the bus without a GLib main loop
}
```

- Rate changes are a seek with the new rate from the current position (verify the exact
  `Element::seek` signature and flags in gstreamer-rs 0.25 docs).
- Track switch: remember the position, set the new `uri`, wait for PAUSED, seek back, resume
  if it was playing. Which file a track uses: Mix = `#AUDIO`, else `#MP3`; Vocals =
  `#VOCALS`; Instrumental = `#INSTRUMENTAL`. Missing tracks are disabled in the UI.
- The bus is polled from the UI frame tick (`poll()` via the bridge), so no GLib main loop
  integration is needed.
- Line playback pads 0.3 s before and after (as in the Factory's note type editor).

### 7.2 Analysis (`analysis.rs`, `cache.rs`)

```rust
pub struct Peaks { pub bucket_ms: f64, pub min: Vec<f32>, pub max: Vec<f32>, pub rms: Vec<f32> }
pub struct Activity { pub bucket_ms: f64, pub active: Vec<bool>, pub floor_db: f32 }
pub fn analyze(path: &Path, bucket_ms: f64, cancel: &AtomicBool, progress: &dyn Fn(f32))
    -> Result<(Peaks, Activity), AudioError>;
```

Pipeline: `uridecodebin ! audioconvert ! audioresample ! audio/x-raw,format=F32LE,channels=1,rate=22050 ! appsink`
(verify caps syntax with `gst-launch-1.0` on a test file). Samples are folded into buckets
as they arrive (`bucket_ms = 10`), so memory stays proportional to the number of buckets, not
the number of samples (a 5 minute song is 30 000 buckets). Activity: RMS in dBFS per 20 ms;
the noise floor is the 10th percentile; active means above floor + 12 dB; gaps shorter than
100 ms are closed. These values are configurable starting points.

Cache: `dirs::cache_dir()/io.github.ueda_ichitaka.UsdxSongEditor/peaks/<key>.bin`, key = hash of
canonical path, size and mtime. A simple versioned binary layout (magic, version, bucket_ms,
count, arrays). A corrupt or old-version cache file is ignored and rewritten.

---

## 8. The app crate (`usdx-song-editor`)

### 8.1 Startup (`main.rs`)

Follow KDE's guide, verified against cxx-qt 0.10 in task P0-03:
`QApplication::new()`, `QGuiApplication::set_desktop_file_name("io.github.ueda_ichitaka.UsdxSongEditor")`,
`QQuickStyle::set_style("org.kde.desktop")` unless `QT_QUICK_CONTROLS_STYLE` is set,
`QQmlApplicationEngine::load(qrc:/qt/qml/<module path>/qml/Main.qml)` (print the exact
resource path the build generates and use it), `app.exec()`.

Command line: `usdx-song-editor [SONG.txt]`, `--version`, `--self-test` (load the QML, create
the window offscreen, exit 0 if the root object exists, non-zero otherwise). Debug builds also
accept `--screenshot <out.png> --demo-state <name> [--song <file>] [--size WxH]`, the
screenshot harness of `Plan/UI_SPEC.md` section 8.1 (task P3-06).

QML module URI: `io.github.ueda_ichitaka.UsdxSongEditor` (verify that cxx-qt accepts the
underscore and capitals in the URI; if not, gate G-04).

### 8.2 Controllers (`src/controller/`, plain Rust)

| Controller | Holds | Tested for |
|---|---|---|
| `EditorController` | `Document`, `Song`, `History`, load snapshot, current voice and line, selection (`BTreeSet<RowId>`), file path, file fingerprint (size, mtime, hash) | every user action maps to the right patch; model change sets; dirty state; navigation |
| `SheetViewport` | `start_beat`, `px_per_beat`, `top_pitch`, `px_per_semitone`, view size | pixel to beat and pitch mapping, zoom around the cursor, drag delta to beats (rounded) |
| `RepairController` | mode, anchors, end pin, options, preview patch, block selection | preview equals what apply commits; cancel leaves the document unchanged |
| `TapController` | session state, taps, latency, pre-roll | key down/up to audio time; undo last tap; result patch |
| `SaveService` (`save.rs`) | backup-once flag | backup written once per session, atomic write (temp file in the same folder, fsync, rename), external change detection |

`EditorController` returns a `ModelChanges` value after each action (which line rows and which
note rows changed, inserted or removed, or "reset") so the bridges emit precise model signals.

### 8.3 Bridges (`src/bridge/`, cxx-qt QObjects)

All are `#[qml_element]` in the module `io.github.ueda_ichitaka.UsdxSongEditor`.

**`SongDocument`** (QObject)
- Properties: `filePath`, `title`, `artist`, `bpm`, `gapMs`, `formatVersion`, `dirty`,
  `readOnly`, `canUndo`, `canRedo`, `undoText`, `redoText`, `lineCount`, `currentLine`,
  `currentVoice`, `voiceCount`, `hasVocals`, `hasInstrumental`, `statusText`.
- Invokables: `open(url) -> bool`, `save() -> bool`, `saveAs(url) -> bool`, `reload()`,
  `undo()`, `redo()`, `goToLine(i)`, `nextLine()`, `previousLine()`, `selectNote(id, mode)`,
  `selectLine()`, `clearSelection()`, `moveSelection(dBeat, dPitch)`,
  `moveSelectionAndFollowing(dBeat)`, `resizeNote(id, edge, dBeat)`, `transposeSelection(n)`,
  `setSelectionType(code)`, `toggleFreestyle()`, `setNoteText(id, text)`,
  `splitNoteAt(id, beat)`, `mergeNote(id)`, `insertNote(beat, pitch)`, `deleteSelection()`,
  `movePhraseEnd(id, beat)`, `togglePhraseEndBeforeSelection()`, `lineTextForEditing(i)`,
  `previewLineText(i, text) -> QString` (JSON of the syllable mapping for the chips),
  `applyLineText(i, text, policy)`, `setHeader(key, value)`, `setGapToMs(ms)`,
  `validationJson() -> QString`, `upgradePreviewJson() -> QString`, `applyUpgrade()`,
  `audioPath(track) -> QString`.
- Signals: `songLoaded()`, `errorOccurred(message)`, `externalChangeDetected()`.

**`LinesModel`** (QAbstractListModel): roles `lineNumber`, `startMs`, `endMs`, `text`,
`typeSummary` (`:` `*` `F` `R` `G` or `mixed`), `health` (0 ok, 1 suspicious, 2 racing),
`healthReason`, `anchor` (empty or `A1`, `A2`...), `isCurrent`.

**`NotesModel`** (QAbstractListModel): the notes of the current line plus one line before and
after. Roles `noteId`, `start`, `length`, `pitch`, `type`, `text`, `lineOffset` (-1, 0, +1),
`selected`, `ghostStart`, `ghostLength` (preview, -1 when none). A second instance in
`PhraseEndsModel` lists the visible phrase ends (`rowId`, `beat`, `min`, `max`).

**`Player`** (QObject, `impl Threading`): properties `positionMs`, `durationMs`, `playing`,
`rate`, `track`, `hasVocals`, `hasInstrumental`; invokables `play()`, `pause()`, `toggle()`,
`seekMs(ms)`, `playRange(startMs, endMs)`, `setRate(r)`, `setTrack(t)`, `tick()` (called
every frame by `FrameAnimation`; updates `positionMs` and handles bus events).

**`SheetBackground`** (`QQuickPaintedItem`): properties `startMs`, `endMs`, `pxPerBeat`,
`startBeat`, `pitchTop`, `pxPerSemitone`, `bpm`, `gapMs`, `showLyricsLane`, `showActivity`,
`overview` (whole-song mode for the repair strip), `peaksReady`, `analysisProgress`, and the
theme colours it needs (passed from QML as `color` properties so it follows the theme). Paints
everything in the note sheet that is not interactive, exactly as `Plan/UI_SPEC.md` section 3
specifies: ruler, pitch rows and labels, grid lines, the lyrics lane, the waveform lane
(including its "Reading the waveform…", "No audio" and "No vocals track" states) and the
activity lane; in overview mode the whole-song strip of UI_SPEC 4.6. Starts analysis on a
worker thread when the audio path changes.

**`Repair`** (QObject): mode (`anchor`, `block`, `tap`), anchors as a JSON list or a small
model, `previewEnabled`, options; invokables `pinCurrentLineAt(ms)`, `setAnchorMs(i, ms)`,
`removeAnchor(i)`, `setEndPin(ms)`, `setBlock(firstLine, lastLine)`, `setBlockTimes(startMs, endMs)`,
`apply()`, `cancel()`, `exportPreviewJson()`, `exportAnchors(url) -> bool`.

**`TapSession`** (QObject): `startSession(firstLine, lastLine, mode, rate)`, `keyDown(ts)`,
`keyUp(ts)`, `undoTap()`, `stop()`, `useTaps()`, `calibrate()`; properties `tapCount`,
`expectedCount`, `currentSyllable`, `latencyMs`.

**`Settings`** (QObject): recent files, latency, health thresholds, default track, autoplay,
last window size.

### 8.4 Drag model

During a drag, the delegate moves visually (`x`/`y` offset). Nothing is committed. On release,
QML calls `moveSelection(dBeat, dPitch)` once with the rounded deltas computed by
`SheetViewport` (an invokable on the sheet or a property set on `SongDocument`), which gives
exactly one undo step per drag. Edge handles call `resizeNote` the same way. Shift locks the
first axis moved; Alt turns off snapping a dragged note's edge to a neighbour's edge.

### 8.5 QML files (`qml/`)

**The look and behaviour of every screen, state and dialog is specified in
`Plan/UI_SPEC.md`, which is binding (decision D-14), with reference images in
`Plan/mockups/`.** This table only maps the files to the UI_SPEC sections they implement.

| File | Content | UI_SPEC |
|---|---|---|
| `Main.qml` | `Kirigami.ApplicationWindow`, `pageStack` (initial `EditorPage`), window title, close guard, drop area, global shortcuts | 2.6, 4.1 |
| `MainMenu.qml` | `Kirigami.GlobalDrawer { isMenu: true }` with the menu items | 4.2 |
| `Metrics.qml` | singleton: every app metric from UI_SPEC (sizes in px or Kirigami units) | 2.1 |
| `Palette.qml` | singleton: note type colours, anchor colour, derived grid colours | 2.3 |
| `EditorPage.qml` | toolbar, `SplitView` of `LinesPane` and the editor column, status bar; the start screen (`Kirigami.PlaceholderMessage` and recent list) while no song is open | 4.1, 4.3 |
| `LinesPane.qml` | `Kirigami.SearchField`, `ListView` with the row delegate | 4.3 |
| `LineHeader.qml` | line header bar | 4.3 |
| `NoteSheet.qml` | `Flickable` with `SheetBackground`, note delegates, phrase end handles, rubber band, playhead; wheel, zoom and pan | 3 |
| `NoteDelegate.qml` | note rectangle, selection border, edge handles, `DragHandler`s, `TapHandler`, ghost outline | 3.5 |
| `PhraseEndHandle.qml`, `Playhead.qml` | | 3.5 |
| `Inspector.qml`, `StatusBar.qml` | | 4.3 |
| `AudioBanner.qml`, `ReadOnlyBanner.qml`, `SaveErrorBanner.qml` | the `InlineMessage`s at the top of the editor column | 4.4, 4.5, 6 |
| `RepairPage.qml`, `OverviewStrip.qml`, `AnchorPanel.qml`, `BlockBar.qml` | repair screens | 4.6, 4.7 |
| `TapAlongPage.qml` | tap-along screen and key handling (`Keys.onPressed`/`onReleased`, ignore auto-repeat) | 4.8 |
| `PropertiesPage.qml` | song properties and checks | 4.9 |
| `SettingsPage.qml` | settings | 4.10 |
| `SaveChangesDialog.qml`, `ChangedOnDiskDialog.qml`, `OpenErrorDialog.qml` | prompt dialogs | 5.1, 5.2, 6 |
| `LineTextDialog.qml`, `ExportAnchorsDialog.qml`, `UpgradeDialog.qml`, `CalibrationDialog.qml` | dialogs | 5.4 - 5.7 |

Every visible string uses `qsTr()` (D-10) with the exact English text from UI_SPEC.

### 8.6 Shortcuts

Implemented as `Kirigami.Action`s with `shortcut`, so they appear in tooltips and menus.
This is the authoritative list (the HTML plan's "Keys and mouse" table shows the same plus
the mouse gestures; UI_SPEC 4.2 lists the menu shortcuts Ctrl+, for Settings and Ctrl+Q for
Quit). Esc closes a dialog, leaves a pushed page, stops playback, or clears the selection, in
that order of precedence.

| Key | Action |
|---|---|
| Space | play / pause |
| P / Shift+P | play line / play selection, 0.3 s padding |
| L | loop line on/off |
| V | next track (Mix, Vocals, Instrumental; skips missing ones) |
| [ / ] | slower / faster (0.5, 0.75, 1.0) |
| Up / Down | previous / next line |
| Left / Right | previous / next note; left of the first note selects the whole line |
| N, G, F, R, Shift+R | normal, golden, freestyle (toggle), rap, rap golden |
| Ctrl+Left / Ctrl+Right | move selection by 1 beat; with Shift also everything after it |
| Ctrl+Up / Ctrl+Down | pitch +1 / -1; with Shift an octave |
| Alt+Left / Alt+Right | length -1 / +1 |
| S / M / Delete / Insert | split at playhead / merge with next / delete / insert after |
| B | phrase end before the selected note on/off |
| Enter / Ctrl+E | edit syllable / edit line text |
| Shift+A | autoplay each line reached |
| A (repair mode) | pin the current line's start to the playhead |
| T | tap-along for the selected lines |
| Ctrl+R | repair timing |
| Ctrl+Z / Ctrl+Shift+Z | undo / redo |
| Ctrl+S / Ctrl+Shift+S / Ctrl+O | save / save as / open |

### 8.7 Saving (`save.rs`)

1. Refuse when the document is read-only.
2. Check the file fingerprint against the one taken at load or last save; if it differs, emit
   `externalChangeDetected` and save only after the user confirms (overwrite, reload, cancel).
3. First save in this session: copy the current file on disk to `<name>.txt.bak` (overwrite).
4. Serialize, write to `<name>.txt.tmp-<pid>` in the same folder, `fsync`, rename over the
   original, `fsync` the folder. On any error the original stays untouched and the temp file
   is removed.
5. `History::mark_saved()`, new fingerprint.

### 8.8 Settings

`dirs::config_dir()/io.github.ueda_ichitaka.UsdxSongEditor/settings.toml`, read at start
(missing or broken file gives defaults and a log warning), written on change. Fields: recent
files (10), latency ms, health thresholds, analysis thresholds, default track, autoplay,
window size.

---

## 9. Testing strategy

### 9.1 Levels

| Level | Where | Tool |
|---|---|---|
| Unit | every module of `usdx-format`, `usdx-edit`, controllers | `#[test]` |
| Property | patches, serializer, retiming invariants, text round trip | proptest |
| Snapshot | serializer output, anchors.txt output, upgrade output, validation lists | insta |
| Integration (audio) | `usdx-audio/tests/` with generated media | cargo test, needs GStreamer |
| App smoke | `usdx-song-editor --self-test` with `QT_QPA_PLATFORM=offscreen` | integration test |
| Corpus | round trip of a real library, opt-in | `USDX_CORPUS=/path` |
| Factory compatibility | exported anchors.txt parsed by the Factory's own code, opt-in | `USDX_FACTORY_DIR=/path` |
| Manual | checklist per UI task (section 10) | human |

### 9.2 Opt-in tests

- `USDX_CORPUS`: a colon-separated list of folders. The test walks them, treats a `.txt` as a
  song file only when it contains a `#BPM` header (as the dashboard does), skips the Factory
  helper files (`lyrics.txt`, `language.txt`, `song_end.txt`, `anchors.txt`, any case) and
  every `.bak` file, and asserts the byte-identical round trip for every song file. Without
  the variable, `scripts/dev.sh corpus` uses the test song folders of section 9.5. Songs are
  never copied into the repository.
- `USDX_FACTORY_DIR`: runs `python3 -I` with `stack/orchestrator` on `sys.path`, calls
  `anchors.read_anchors(dir)` on an exported file and `anchors.match_anchor_units(...)` against
  a fixture `lyrics.txt`, and asserts no exception and the expected indices.

### 9.3 Fixtures (`src/usdx-format/tests/fixtures/`, handwritten)

`minimal_v1.txt`, `unversioned_crlf.txt`, `bom_utf8.txt`, `cp1252_unversioned.txt` (with
umlauts and bytes 0x80 to 0x9F), `encoding_header_cp1250.txt`, `relative_mode.txt`,
`duet_p1_p2.txt`, `trailing_after_e.txt`, `no_e.txt`, `unknown_note_type.txt`,
`mixed_line_endings.txt`, `no_final_newline.txt`, `tabs_as_separators.txt`,
`comma_decimal_bpm.txt`, `leading_trailing_spaces.txt`, `ultrasinger_1_2_0.txt` (header layout
like UltraSinger's, invented lyrics), `overlaps_and_breaks.txt` (validation cases),
`invalid_rows.txt`, `header_case_and_duplicates.txt`.

### 9.4 Audio test media

Generated per test run into a `tempfile::TempDir` with GStreamer pipelines (in
`testmedia.rs`): 5 s 440 Hz tone with a 1 s silence in the middle, as Ogg/Opus
(`audiotestsrc ! opusenc ! oggmux ! filesink`) and MP3 (`lamemp3enc ! id3v2mux`). Exact
pipelines are verified with `gst-launch-1.0` first.

### 9.5 Real test songs (USDX Song Factory output)

The user approved these folders for testing (12 November 2026):

| Folder | Songs | What they cover |
|---|---|---|
| `/home/ueda/workspace/USDX-Song-Factory/stack/output/new/` | 105 | Generated from scratch. Audio: 97 opus, 8 mp3. 97 have `#VOCALS` and `#INSTRUMENTAL`. 95 have `lyrics.txt` beside them. Note types `:` `*` `F` `R` `G` all present |
| `/home/ueda/workspace/USDX-Song-Factory/stack/output/repaired/recreated/` | 20 | Re-created by the Factory. Audio mp3, all with `#VOCALS`. Normal notes only |
| `/home/ueda/workspace/USDX-Song-Factory/stack/output/repaired/sync/` | 11 | Repaired in sync mode. Audio mp3, no `#VOCALS`. Golden notes. One folder holds two song files |

Not to be used: `stack/output/repaired/gap/` and every other folder under `stack/output/`.

Profile on 12 November 2026: all 136 song files are UTF-8 without BOM, LF line endings,
`#VERSION:1.2.0`, end with `E`, and none is a duet. Encoding, BOM, CRLF, relative-mode,
duet and invalid-row cases therefore come only from the handwritten fixtures (9.3).

Rules for using them:

- Read-only. No test, script or agent writes into these folders. A test that saves, exports
  or upgrades copies the song folder's `.txt` (and `lyrics.txt` if needed) into a
  `tempfile::TempDir` first and works on the copy. The audio stays where it is and is opened
  by absolute path from the temp copy's test harness, never copied (the folders hold several
  GB of media).
- Tests that use them are opt-in (`#[ignore]` plus an environment variable), so
  `scripts/dev.sh check` stays fast and works without the Factory repository.
- Memory and time: never decode all audio in one run. Audio tests take at most
  `USDX_AUDIO_SAMPLE` songs (default 3), picked by a fixed list of folder names written into
  the test, so runs are reproducible.

Uses per phase:

| Task | Use |
|---|---|
| P1-09 | Round trip of all 136 song files (`USDX_CORPUS`) |
| P1-08 | Validation over all songs: no errors expected; the Info about `1.2.0` on every file |
| P4-02, P4-03 | Opt-in player and analysis tests on real opus and mp3 files (`USDX_AUDIO_SAMPLE`) |
| P7-05, P7-10 | Health analysis over all songs with `#VOCALS`: the distribution of syllable rates and vocal coverage is the basis for the thresholds |
| P7-06 | Anchors export on copies of songs from `new/` with their `lyrics.txt`: the lyrics.txt matching port must agree with the Factory's own `match_anchor_units` for every line of every song (opt-in, needs `USDX_FACTORY_DIR`) |
| P7-02 | Synthetic-fault acceptance test repeated on a fixed list of 10 real songs from `new/`: compress a section, repair with the original anchors, compare within 1 beat |

---

## 10. Tasks by phase

Format of each task: ID, title, Needs, Files, Tests first (RED), Notes, Done when.
Mark progress in `Plan/PROGRESS.md`.

### Phase 0 - Toolchain and skeleton

**P0-01 Workspace**
- Needs: none.
- Files: `Cargo.toml`, `src/usdx-format`, `src/usdx-edit`, `src/usdx-audio` (empty libs with
  headers), `rustfmt.toml` (edition 2024), `.gitignore` additions (`target/`, `build/`).
- Tests first: one `#[test] fn crate_builds()` per lib crate (fails before the crates exist).
- Done when: `~/.cargo/bin/cargo test --workspace` passes.

**P0-02 Guarded dev script**
- Needs: P0-01.
- Files: `scripts/dev.sh`, `scripts/tests/dev_guard_test.sh`.
- Tests first: the guard functions (`jobs_for`, `mem_ok`, `memory_max`) are sourced and
  tested with fake meminfo files: 2 GiB refuses, 8 GiB with 16 threads gives 4 jobs, 64 GiB
  with 16 threads gives 8 jobs.
- Done when: all subcommands of section 2.5 work and the guard tests pass.

**P0-03 App skeleton**
- Needs: P0-02.
- Files: `src/usdx-song-editor/{Cargo.toml,build.rs,src/main.rs}`, `qml/Main.qml`.
- Tests first: `tests/self_test.rs` runs the binary with `--self-test` and
  `QT_QPA_PLATFORM=offscreen`, expects exit 0; runs `--version`, expects the crate version.
- Notes: confirm every call of 8.1 compiles on cxx-qt 0.10; if an API differs from the KDE
  guide, use the 0.10 API and note it in PROGRESS.md. If cxx-qt 0.10 cannot build the skeleton
  at all, stop: gate G-02.
- Done when: `scripts/dev.sh run` shows an empty Kirigami window with the Breeze style on
  Plasma (manual check) and the test passes.

**P0-04 CMake install and desktop integration**
- Needs: P0-03.
- Files: `CMakeLists.txt`, `packaging/*.desktop`, `packaging/*.metainfo.xml`, `packaging/*.svg`.
- Tests first: a test script builds with `cmake -B build -DCMAKE_INSTALL_PREFIX=<tmp>`,
  installs into the temp prefix, and checks that the binary, the desktop file and the icon
  exist; `desktop-file-validate` passes if installed (skip with a message if not).
- Done when: install into a temp prefix works; the window icon shows on Wayland (manual).

### Phase 1 - `usdx-format`

**P1-01 Byte layer** (5.2). Tests first: BOM detection, UTF-8, `#ENCODING` header, guessed
CP1252, exact `encode(decode(b)) == b` for all terminator kinds, empty file, file without final
newline, windows-1252 bytes 0x80 to 0x9F.

**P1-02 Row classification** (5.3). Tests first: one test per rule and per ABNF example in the
spec; tab separators; leading spaces in text preserved; `P 1`; `E` with trailing content;
`#` line in the body is `Invalid`; negative start parsed with a flag.

**P1-03 Headers** (5.1, 5.4). Tests first: case-insensitive lookup, first occurrence wins with
a duplicate warning, trimming, empty value equals absent, decimal comma, `#VERSION` parsing
(unversioned, 1.0.0, 1.1.0, 1.2.0, 2.0.0, garbage).

**P1-04 Timing** (5.5). Tests first: the values from the sample song; round trip
`ms_to_beat(beat_to_ms(b)) == b` within 1e-9; `None` without BPM or with BPM 0.

**P1-05 Song view** (5.4). Tests first: lines split at `-`, `Pn`, `E`; implicit P1; duet with
two voices; line text joined from syllables; rebuild time below 1 ms for a generated
3000-row song in release mode (an `#[ignore]` benchmark-style test run by `dev.sh check --release`).

**P1-06 Relative mode, reading** (5.1). Gate G-01 applies to the exact semantics: the
unversioned spec says `rel` starts at the value of `#GAP`; verify this against the USDX
implementation (web source, read-only) before writing the test, and ask the user if it is
still unclear. Tests first: fixture `relative_mode.txt` gives the expected absolute beats.

**P1-07 Serializer** (5.6). Tests first: round trip for every fixture (insta snapshot of
nothing-changed is the input bytes); dirty note, dirty phrase end, changed header value keeps
the raw key spelling, new header inserted after the last header with the dominant terminator;
proptest: generated documents round-trip.

**P1-08 Validation** (5.7). Tests first: one fixture or inline song per issue kind; the
`ultrasinger_1_2_0.txt` fixture yields exactly the Info about 1.2.0 and no error.

**P1-09 Corpus test** (9.2, 9.5). Done when all 136 song files in the test song folders
round-trip byte-identical.

### Phase 2 - `usdx-edit` core

**P2-01 Patch** (6.1). Tests first: replace, insert at start, insert after, remove; inverse;
proptest for apply/inverse identity; applying to a document without the target row fails
cleanly.

**P2-02 History** (6.2). Tests first: undo/redo labels, redo cleared by a new commit, dirty
after commit, clean after undo back to the saved position, dirty again after redo.

**P2-03 Note operations** (6.3). One test group per function in the table, including the
error cases (`BeforeSongStart`, length below 1, merge across a phrase end, phrase end out of
its range). Each test also checks that only the expected rows became dirty.

**P2-04 Header operations** (6.4). Tests first: `set_bpm` with `keep_times` keeps every note
within half a beat of its old time; `set_gap_ms` changes only the GAP row.

**P2-05 Change log for export** (`changes.rs`, 6.11). Tests first: no changes after load; a
moved first note marks its line; a pitch change does not; undo back removes the mark.

### Phase 3 - App shell

**P3-01 EditorController: load and navigate** (8.2). Tests first: load a fixture, line count,
current line moves and clamps, `goToLine`, read-only for major version 2.

**P3-02 Save service** (8.7). Tests first (temp dir): backup written on first save only,
`.bak` content equals the file before the first save, atomic write leaves the original intact
when serialization fails (inject an error), external change detected by size/mtime/hash.

**P3-03 Settings** (8.8). Tests first: defaults when missing; broken TOML gives defaults and
a warning; recent files capped at 10 and deduplicated.

**P3-04 SongDocument and LinesModel bridges** (8.3). Tests: controller tests cover the logic;
the bridge gets a smoke test through `--self-test` loading a fixture path given on the command
line (`--self-test path/to/song.txt` must report the line count on stdout).

**P3-05 QML shell** (UI_SPEC 2, 4.1, 4.2, 4.3 without the sheet, 4.5, 5.1 - 5.3, 6):
`Main.qml`, `Metrics.qml`, `Palette.qml`, `MainMenu.qml`, `EditorPage.qml` with toolbar,
start screen, `SplitView`, `LinesPane.qml`, `LineHeader.qml`, `StatusBar.qml`, the open
`FileDialog`, recent songs, window title rules, close guard and `SaveChangesDialog`,
`ChangedOnDiskDialog`, `OpenErrorDialog`, `ReadOnlyBanner`, `SaveErrorBanner`, passive
notifications, drag and drop. First verify that the `isMenu` GlobalDrawer opens from our own
toolbar button (gate G-08 if not). Controller tests for title text, enablement (UI_SPEC 7) and
the dialog flows (save then continue; failed save stops). Acceptance: UI_SPEC 8.2 for
`start-screen`, `main-menu`, `read-only`, `unsaved-changes`, `changed-on-disk`,
`errors-and-notifications`.

**P3-06 Screenshot harness** (UI_SPEC 8.1). Needs P3-05. `--screenshot`, `--demo-state`,
`--song`, `--size` in debug builds; the theme switch through a temporary `XDG_CONFIG_HOME`
with `kdeglobals` copied from the Breeze colour schemes; `scripts/ui-shots.sh` (memory guard,
both themes, output in `target/ui-shots/`). Tests first: an integration test renders the
`start-screen` state in both themes and asserts that the PNG exists, has the requested size,
and that the pixel at (5, 5) has the window background of the theme (`#eff0f1` light,
`#202326` dark). Demo states of later screens are added by the task that builds the screen.

### Phase 4 - Audio

**P4-01 Test media helper** (9.4). Tests first: generated opus and mp3 files exist and have a
duration of 5 s within 50 ms.

**P4-02 Player** (7.1). Tests first: load, play reaches PLAYING, `seek_ms(2000)` gives
`position_ms` within 10 ms after the seek completes (opus and mp3), `play_range(1000, 1500)`
ends with `RangeDone` within 100 ms of 1500, rate 0.5 makes position advance at about half
speed (measured over 1 s, tolerance 15 %), missing file gives an error.

**P4-03 Analysis and cache** (7.2). Tests first: bucket count equals duration / bucket_ms
(plus or minus 1); the silent second is inactive and the tone is active; cancel stops within
200 ms; cache hit returns identical data; changed mtime invalidates; corrupt cache file is
ignored.

**P4-04 Player bridge and playhead**: `Player` QObject, `FrameAnimation` calls `tick()`;
track and speed combo boxes in the toolbar (UI_SPEC 4.3); line playback with padding; loop;
`AudioBanner` and the audio states (UI_SPEC 4.4). Manual checklist: playhead stays in sync
over a whole song at 1x and 0.5x; Vocals and Instrumental switch without a jump. Acceptance:
UI_SPEC 8.2 for the three `audio-states` demo states.

### Phase 5 - Note sheet and mouse editing

**P5-01 SheetViewport** (8.2). Tests first: beat/pixel and pitch/pixel round trips, zoom
around the cursor keeps the beat under the cursor, drag delta rounding, visible range.

**P5-02 SheetBackground** (8.3, UI_SPEC 3). Tests: the painting code computes draw lists in a
plain function (`fn layout(viewport, peaks, lines, colours, ...) -> Vec<DrawOp>`), which is
unit-tested against the numbers of UI_SPEC 3: ruler 22 px, lyrics lane 34 px with the
two-row placement rule (test the "or"/"just" case from the reference song: beats 189 and 192
must land in different rows), 8 px gap, waveform 54 px, activity 6 px, gutter 46 px, pitch
range and row height rules, label rules, tick interval choice. `paint` only replays the list.

**P5-03 NotesModel and PhraseEndsModel**. Tests first: the visible set for a line includes its
neighbours; model change sets after move, insert, delete, line change.

**P5-04 Note delegates: select, drag, resize, rubber band** (8.4). Controller tests for the
resulting calls; manual checklist: one undo step per drag; Shift axis lock; Alt; Ctrl+click.

**P5-05 Phrase end handle, insert, split, merge, delete, type keys, inspector**. Controller
tests per action; manual checklist per shortcut of 8.6.

**P5-06 Keyboard navigation and autoplay**. Controller tests for Left/Right semantics (left of
the first note selects the line), Up/Down, autoplay triggering `playRange` with padding.
Acceptance for phase 5 as a whole: UI_SPEC 8.2 for `editor`.

### Phase 6 - Text

**P6-01 Syllable parser and plan** (6.5). Tests first: parser cases (spaces, `|`, `~`,
punctuation), round trip with `line_text_for_editing`, split and merge plans keep the total
beats, `Ask` policy returns the proposal.

**P6-02 Inline syllable edit and LineTextDialog** (UI_SPEC 3.6, 5.4). The chip JSON from
`previewLineText` is unit-tested in the controller. Acceptance: UI_SPEC 8.2 for `line-text`.

### Phase 7 - Timing repair

**P7-01 Collision pass** (6.6). Property tests for all invariants; the GAP shift case.

**P7-02 Anchor + stretch** (6.7). Tests first: two anchors, three anchors, `Outside` and
`Lengths` variants, end pin, out-of-order anchors error, the synthetic-fault acceptance test.

**P7-03 Block transform** (6.8). Tests first: move only, scale only, `MoveAlong` versus `Stay`.

**P7-04 Tap mapping** (6.9). Tests first: line mode equals anchor stretch with the same times;
syllable mode with and without held keys; fewer taps than notes.

**P7-05 Health analysis** (6.10). Tests first: a normal song is all Ok; the synthetic racing
section is Racing; coverage signal with a fake activity vector; `racing_sections` merging.

**P7-06 Anchors export** (6.11). Tests first: time formatting (`1:06.18`, `0:05.00`,
`61:00.00` is written as `1:01:00.00`), text normalisation, content rule D-12, sorting and
deduplication, end line, lyrics.txt matching port (cases: not found, chorus bound to a later
occurrence, all fine), insta snapshot of a full file; opt-in Factory compatibility test.

**P7-07 RepairController and TapController** (8.2). Tests first: preview equals the committed
patch; cancel restores; tap timestamps minus latency; undo last tap; auto-repeat key events
ignored.

**P7-08 Repair UI** (UI_SPEC 4.6 - 4.8, 5.5, 5.7): `RepairPage`, `OverviewStrip`,
`AnchorPanel`, `BlockBar`, ghost notes, `TapAlongPage`, `CalibrationDialog` (writes a
click-track WAV with 8 clicks at 1 s spacing into the cache directory with a small WAV writer,
no new dependency), `ExportAnchorsDialog`. Manual checklist following the "Repair a racing
section" workflow in the HTML plan. Acceptance: UI_SPEC 8.2 for `repair-anchor`,
`repair-block`, `tap-along`, `latency-calibration`, `export-anchors`.

**P7-09 Line health in the lines pane and editor status**. Manual check.

**P7-10 Threshold tuning**. Run health analysis over the test songs with `#VOCALS` (9.5) and
report the distributions (syllable rate, neighbour factor, vocal coverage) to the user. Then,
with the user: compare against songs the user names as broken and as good, adjust the
defaults, and record the numbers in PROGRESS.md. Gate G-06.

### Phase 8 - Properties, upgrade, packaging

**P8-01 Upgrade planner** (6.12). Tests first: fixtures unversioned CP1252, BOM + CRLF,
`#RELATIVE:no`, `1.0.0`, `1.2.0` (version stays), missing `#AUDIO`; insta snapshots of the
output bytes and the change lists.

**P8-02 PropertiesPage, validation list, UpgradeDialog** (UI_SPEC 4.9, 5.6). Gate G-05 first.
Acceptance: UI_SPEC 8.2 for `song-properties` and `upgrade`.

**P8-03 Settings page** (UI_SPEC 4.10). Acceptance: UI_SPEC 8.2 for `settings`.

**P8-04 Packaging**: `packaging/PKGBUILD` (builds with cargo in release mode through CMake,
installs to `/usr`), metainfo validated with `appstreamcli validate` if installed. Test: a
`makepkg` dry run in a temp directory is a manual step for the user (agents do not install
system packages).

**P8-05 Documentation**: `Docs/user-manual.md` (opening, editing, repair workflows, keys,
export, what saving does), `README.md` (build, run, test, corpus test).

### Phase 9 - Later (each item needs the user's go-ahead first)

- P9-01 Duet editing: voice switcher, all controllers voice-aware, export per voice.
- P9-02 Video preview with `#VIDEOGAP`.
- P9-03 Detected vocal pitch over the notes.
- P9-04 Import of the Factory's note type marks files (`stack/input/note-types/*.tsv`).
- P9-05 German translation (`qsTr` catalogs; needs a `QTranslator` binding, not in cxx-qt-lib
  0.10, so a small bridge).
- P9-06 Editing relative-mode songs in relative form (see G-01).

---

## 11. Gates: questions that must go to the user

Stop and ask when a task reaches one of these. Do not choose an answer.

| ID | When | Question |
|---|---|---|
| G-01 | P1-06, P8-01 | Relative mode: the spec says `rel` starts at `#GAP`; if the USDX implementation differs, which one to follow? Should edits to a relative-mode song be saved in relative form, or must the user convert it to absolute first? |
| G-02 | P0-03 | If cxx-qt 0.10 cannot build the skeleton: fall back to 0.8.1 (the KDE guide's version) or investigate further? |
| G-03 | any | A needed crate is not in section 2.4 |
| G-04 | P0-03 | If the QML module URI `io.github.ueda_ichitaka.UsdxSongEditor` is not accepted: which URI instead? |
| G-05 | P8-02 | Properties page: plain `Kirigami.FormLayout` or Kirigami Addons `FormCard`? |
| G-06 | P7-10 | Which songs to use for threshold tuning, and are the tuned values acceptable? |
| G-07 | any | Any behaviour this plan or `Plan/UI_SPEC.md` does not define |
| G-08 | P3-05 | If a `GlobalDrawer` with `isMenu: true` cannot be opened from our own toolbar button: which component to use for the main menu instead? |

---

## 12. Reference material (read-only)

Paths outside the repository that agents may read, and nothing else outside it:

- `/home/ueda/workspace/USDX-Song-Factory/utilities/note_type_editor.py`,
  `utilities/note_types.py`, `docs/note-types.md`: note type keys and line splitting rules.
- `/home/ueda/workspace/USDX-Song-Factory/stack/orchestrator/anchors.py`,
  `stack/orchestrator/outro.py` (`parse_time`), `stack/orchestrator/test_anchors.py`,
  `docs/repairs.md` (section "Anchors"): anchors.txt format and matching.
- `/home/ueda/workspace/USDX-Song-Factory/knowledge/05-LESSONS.md`: synthetic fault testing.
- `/home/ueda/workspace/karaoke-dashboard/app/format_check.py`, `app/ultrastar.py`: validation
  categories, Factory helper file names.
- Test songs (section 9.5, read-only, never copied into this repository):
  `/home/ueda/workspace/USDX-Song-Factory/stack/output/new/`,
  `/home/ueda/workspace/USDX-Song-Factory/stack/output/repaired/recreated/`,
  `/home/ueda/workspace/USDX-Song-Factory/stack/output/repaired/sync/`.
  Not `stack/output/repaired/gap/`.
- `/home/ueda/workspace/karaoke-dashboard/songs-sample/`: only for looking at real file
  layouts; the test corpus is the folders above.
- Web: `https://github.com/UltraStar-Deluxe/format`, `https://usdx.eu/format/`,
  `https://develop.kde.org/docs/getting-started/kirigami/setup-rust/`,
  `https://github.com/KDAB/cxx-qt` (tag `v0.10.0`), docs.rs for every crate in 2.4,
  `https://gstreamer.freedesktop.org/documentation/`, `https://github.com/SarutaSan72/Yass`
  (feature ideas only; GPL-3.0, do not copy code).
- Downloaded crate sources in `~/.cargo/registry/src/` for the pinned versions.
