# Handoff - USDX Song Editor (16 November 2026)

About me: the hand-over note for the next agent picking up this repository mid-phase 7. It
says where the work stands, what is uncommitted, what is open, and the working rules and
gotchas learned so far. `Plan/PROGRESS.md` stays the authoritative task record; this file is
the short entry point. Delete or replace it when the open items below are done.

## 1. Read first

1. `~/.claude/CLAUDE.md` (the user's rules) and `Plan/IMPLEMENTATION_PLAN.md` section 0
   (TDD red-green-refactor with a real RED run, ask on every open option, no guessing, NO git
   commands - the user commits, stay inside the repo except the paths in plan section 12,
   about-me header in every file, comments describe the present only, no emojis except
   U+2705/U+274C in tables, never the en dash, build/test only through `scripts/dev.sh`).
2. `Plan/PROGRESS.md` rows P7-01 to P7-07 and R-04/R-05 (the recent rounds, user decisions
   and gotchas of the timing-repair phase).
3. `Plan/UI_SPEC.md` (binding), sections 4.6-4.8, 5.5, 5.7 for the coming repair UI.

Paths the user allowed outside the repo: `/home/ueda/workspace/USDX-Song-Factory/` (test songs
in `stack/output/new` and `stack/output/repaired/{recreated,sync}`, plus the reference files of
plan section 12) and `/home/ueda/workspace/karaoke-dashboard/` (the Realize sample song). Open
songs only as copies; never write to the user's files.

## 2. State

- Phases 0-6 done; rounds R-02 to R-05 done. The corpus gate is closed: the clean full
  self-test sweep passed 136 of 136 songs, 0 failures (`/tmp/opencode/sweep10.log`; the sweep
  copies and script live in the session scratchpad
  `/tmp/claude-1000/-home-ueda-workspace-USDX-Song-Editior/4cda98e0-4448-4514-af0a-aa5e02519be4/scratchpad/`,
  run under `systemd-run --user --scope -q -p MemoryMax=6G`, ~25-30 min, never rebuild the
  app binary while a sweep runs).
- Phase 7 (timing repair): P7-01 to P7-07 done (collision pass, anchor + stretch, block
  transform, tap mapping, health analysis, anchors export, RepairController + TapSession).
- Last full `scripts/dev.sh check`: green, 722 passed (2026-11-16).
- `cargo` is not on the shell PATH: use `~/.cargo/bin/cargo fmt --all`; everything else runs
  through `scripts/dev.sh check | test <crate> [--test name] | build | corpus`.

### Uncommitted files (for the user to commit; last commit `a455b41`)

| File | Content |
|---|---|
| `src/usdx-edit/src/block.rs` (new) + `tests/block.rs` | P7-03 block transform |
| `src/usdx-edit/src/tap.rs` (new) + `tests/tap.rs` | P7-04 tap mapping |
| `src/usdx-edit/src/health.rs` (new) + `tests/health.rs` | P7-05 health analysis |
| `src/usdx-edit/src/anchors_export.rs` (new) + `tests/anchors_export.rs` | P7-06 anchors.txt export |
| `src/usdx-edit/Cargo.toml`, `Cargo.lock` | usdx-audio path dep (health analysis) |
| `src/usdx-edit/src/lib.rs` | module registrations |
| `src/usdx-edit/src/retime.rs` | Debug + PartialEq derives on `Anchor`, `Pin` |
| `src/usdx-edit/tests/header_ops.rs`, `ops.rs` | `Vec::<T>::new()` where the usdx-audio dep made bare `vec![]` ambiguous |
| `src/usdx-song-editor/src/controller/tap.rs` (new) + `tests/tap.rs` | P7-07 TapSession |
| `src/usdx-song-editor/src/controller/repair.rs` (new) + `tests/repair.rs` | P7-07 RepairController |
| `src/usdx-song-editor/src/controller/mod.rs` | module registrations |
| `src/usdx-song-editor/cpp/selftest.cpp` | guide-drag fix (first uncovered candidate; R-05 finding 6) |
| `Plan/PROGRESS.md`, `Plan/HANDOFF.md` | records |

## 3. Open items, in order

- **P7-08 Repair UI** (plan 8.3 invokables `Repair`/`TapSession`, UI_SPEC 4.6-4.8): the
  QObject bridges over `controller/repair.rs` and `controller/tap.rs` and the QML pages.
- **P7-09** line health in the lines pane and editor status (`usdx-edit::health`).
- **P7-10** threshold tuning, gate G-06 (with the user).
- **P8-01 to P8-05** upgrade planner, properties page, settings page, packaging, docs.

User decisions that bind the repair UI: the controllers are standalone structs taking
`&mut LoadedSong`; the Repair QObject's mode enum reads anchor/block/tap verbatim with the
TapSession inside the controller; the anchors export is pure (the dialog code reads
lyrics.txt and passes its lines); inline expected text instead of an insta dep.

## 4. Open questions for the user (never answered yet)

- Should the first save overwrite an existing `<song>.txt.bak` that the editor did not write?
  It currently does.
- Known, not fixed: stacked identical notes in Factory output (only the top one is
  clickable); the start-edge preview at beat 0 shows a length the op then refuses;
  notifications cover the waveform lane for 2-4 s.

## 5. Gotchas worth knowing

- Self-test (`cpp/selftest.cpp`, `src/lib.rs`): runs before the app event loop; synthesized
  input through `QWindowSystemInterface` with synchronous delivery plus `pump_events`; reach
  delegates through the Repeaters' `itemAt` (`live_delegate`); a check that cannot run on a
  real song prints "skipped ..." (never "ok"), and the multi-note test in
  `tests/self_test.rs` requires every "...: ok" line. Pick click targets inside the visible
  sheet (right of the gutter, inside the pitch rows): notes left of the view sit clipped
  under the lines pane. A later sibling delegate can cover a candidate note's centre, so a
  press lands on the covering note - the guide-drag step picks the first candidate no later
  delegate covers.
- Qt logs go to the journal when stderr is not a terminal: set `QT_FORCE_STDERR_LOGGING=1`
  to see `console.warn` output of a direct run.
- QML: inline components cannot see outer ids (pass values as properties); a hidden text
  field keeps the focus (hand it back, or Ctrl+Z and the arrows stay disabled); the desktop
  style draws a default button from `Accessible.defaultButton`, not `highlighted`.
- `f64::clamp` panics on NaN; a viewport without size can have a NaN zoom (guarded in
  `SheetViewport::clamp_start`).
- The 1280 px toolbar is full; new controls need another place.
- Screenshots: `scripts/ui-shots.sh [states]`, compare against `Plan/mockups/` (UI_SPEC 8.2);
  caret blink and the "Main menu" tooltip fade differ between runs.
- Never rebuild while a sweep runs: the sweep calls the binary per song.
- Rust: `f64::from(i64)` does not exist (`as f64`); `&RowId == RowId` needs a deref in
  closure finds; adding a dep that pulls glib/gstreamer makes bare `vec![]` comparisons
  ambiguous (`Vec::<T>::new()`); `StretchOptions` has no `Default` (build the explicit
  Keep/Keep/Keep/None literal); `FileFingerprint::new(size, modified, &bytes)` is the
  constructor; clippy wants `notes.clone()` over `.iter().copied().collect()` and elided
  lifetimes, and its doc-list lint reads a wrapped "+ neighbours" at a line start as a
  markdown list; `usize::from(voice)` is the voices index, `voice.number` is 1-based.
