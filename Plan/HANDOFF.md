# Handoff - USDX Song Editor (17 November 2026)

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

- Phases 0-7 done; rounds R-02 to R-05 done. P7-10 (2026-11-18): tuned health defaults
  (factor 2.0, coverage racing off), the past-end check with the audio length probed at
  open (`usdx_audio::duration_ms`, new dependency gstreamer-pbutils), the user's labels in
  `src/usdx-edit/tests/health_labels.tsv` with an opt-in report (PROGRESS P7-10).
- Last full `scripts/dev.sh check`: green, 802 passed, 0 failed, 10 ignored (2026-11-18).
- Last full corpus sweep: 133 of 136, the 3 failures fixed and re-run with 10 more songs
  (PROGRESS P7-08).
- `cargo` is not on the shell PATH: use `~/.cargo/bin/cargo fmt --all`; everything else runs
  through `scripts/dev.sh check | test <crate> [--test name] | build | corpus`.
- Corpus self-test sweep: copies of the 136 corpus songs (songs only, no audio) in
  `/tmp/claude-1000/-home-ueda-workspace-USDX-Song-Editior/4cda98e0-4448-4514-af0a-aa5e02519be4/scratchpad/sweep/<n>/song.txt`,
  script `.../scratchpad/sweep.sh <sweep dir> <binary>`, run under `systemd-run --user
  --scope -q -p MemoryMax=6G`; about 40 s per song now (1.5 h for all), so never rebuild the
  app binary while it runs (`cargo test -p usdx-song-editor` rebuilds it). If the scratchpad
  is gone, rebuild the copies from the three corpus folders.
- Demo states (UI_SPEC 8.1) in `scripts/ui-shots.sh`: start-screen, editor, line-text,
  repair-anchor, repair-block, tap-along, export-anchors, latency-calibration and the three
  audio states.

### Uncommitted files (last commit `52f814a repair song pages`)

Everything under `git status`: the P7-08 bridges and QML (RepairPage, RepairSheet,
AnchorPanel, BlockBar, TapAlongPage, ExportAnchorsDialog, CalibrationDialog), the
controller modules calibration.rs and line_health.rs, usdx-edit's health signals and
ExportProblem, usdx-audio's `cached_activity`, the screenshot and self-test helpers, the
tests, PROGRESS and this file.

## 3. Open items, in order

- **R-07** done (2026-11-18): Shift+drag / Shift+click / the toolbar's Add note place new
  notes (`insert_note_at`); the toolbar turns five buttons icon-only when narrow.
- **R-06** done (2026-11-18): the inspector's type buttons commit again, notes show their
  syllable, the line text is edited in the lyrics lane (LineTextEditor.qml, the dialog is
  gone).
- **Phase 7b (P7-11 to P7-16)** offset and missing-line detection from a vocal pitch track,
  drafted 2026-11-18 in the plan; deferred by the user (2026-11-18), questions at gate G-09.
- **P8-01 to P8-05** upgrade planner, properties page, settings page, packaging, docs.

User decisions that bind the repair UI: the controllers are standalone structs taking
`&mut LoadedSong`; the Repair QObject's mode enum reads anchor/block/tap verbatim with the
TapSession inside the controller; the anchors export is pure (the dialog code reads
lyrics.txt and passes its lines); inline expected text instead of an insta dep; a repair's
collision pass reaches only the retimed lines and their neighbours.

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
- Kirigami's PageRow creates a pushed Component under a QtObject parent, which Qt warns
  about (and the self-test fails on warnings): push pages as items created once with an
  Item parent (Main.qml `pushPageItem`); a popped item page goes back hidden.
- The Repair bridge mirrors the document only when one of its invokables runs: call
  `repair.refresh()` after anything that changes the song outside the repair page.
- Rust: `f64::from(i64)` does not exist (`as f64`); `&RowId == RowId` needs a deref in
  closure finds; adding a dep that pulls glib/gstreamer makes bare `vec![]` comparisons
  ambiguous (`Vec::<T>::new()`); `StretchOptions` has no `Default` (build the explicit
  Keep/Keep/Keep/None literal); `FileFingerprint::new(size, modified, &bytes)` is the
  constructor; clippy wants `notes.clone()` over `.iter().copied().collect()` and elided
  lifetimes, and its doc-list lint reads a wrapped "+ neighbours" at a line start as a
  markdown list; `usize::from(voice)` is the voices index, `voice.number` is 1-based.
