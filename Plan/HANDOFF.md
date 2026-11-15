# Handoff - USDX Song Editor (15 November 2026)

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
2. `Plan/PROGRESS.md` rows R-02, R-03, R-04, P6-01, P6-02 and P7-01 (recent rounds, user
   decisions, gotchas).
3. `Plan/UI_SPEC.md` (binding), sections 4.6-4.8, 5.5, 5.7 for the coming repair UI.

Paths the user allowed outside the repo: `/home/ueda/workspace/USDX-Song-Factory/` (test songs
in `stack/output/new` and `stack/output/repaired/{recreated,sync}`, plus the reference files of
plan section 12) and `/home/ueda/workspace/karaoke-dashboard/` (the Realize sample song). Open
songs only as copies; never write to the user's files.

## 2. State

- Phases 0-6 done; rounds R-02 (playhead, bug hunt), R-03 (drag flicker, bug hunt) and R-04
  (scroll limit, Go to song start/end, End song here with #END, drag guides) done and
  committed by the user (last commit `9321b52`).
- Phase 7 (timing repair) started: P7-01 in progress, P7-02 to P7-10 todo.
- `cargo` is not on the shell PATH: use `~/.cargo/bin/cargo fmt --all`; everything else runs
  through `scripts/dev.sh check | test <crate> [--test name] | build | corpus`.
- Last full `scripts/dev.sh check`: green, 643 passed, 0 failed, 5 ignored (before the P7-01
  files and the latest self-test edits; rerun it first).

### Uncommitted files (for the user to commit after the open items)

| File | Content |
|---|---|
| `src/usdx-edit/src/collisions.rs` (new) | P7-01 collision pass |
| `src/usdx-edit/tests/collisions.rs` (new) | 13 tests, 2 proptests, green |
| `src/usdx-edit/tests/repair_corpus.rs` (new) | opt-in corpus report of the collision pass |
| `src/usdx-edit/src/lib.rs` | `pub mod collisions;` |
| `scripts/dev.sh` | `corpus` also runs `repair_corpus` |
| `src/usdx-song-editor/cpp/selftest.cpp` | self-test fixes from the R-04 sweep (see 3.1) |
| `Plan/PROGRESS.md`, `Plan/HANDOFF.md` | records |

## 3. Open items, in order

### 3.1 Finish the R-04 corpus sweep (one song left)

The sweep runs the app's `--self-test` on copies of all 136 corpus songs. The copies live in
the session scratchpad of the previous agent
(`/tmp/claude-1000/-home-ueda-workspace-USDX-Song-Editior/4cda98e0-4448-4514-af0a-aa5e02519be4/scratchpad/sweep/<n>/song.txt`,
script `.../scratchpad/sweep2.sh <sweep dir> <binary>`); if that folder is gone, rebuild it by
copying every song file of the three corpus folders into its own numbered folder (songs
only, no audio) and loop `QT_QPA_PLATFORM=offscreen timeout 120 <binary> --self-test
<copy>` under `systemd-run --user --scope -q -p MemoryMax=6G`.

- Fixed in `selftest.cpp`, not yet rerun over all songs: wheel until the view stops instead
  of 60 steps (97 failures on long songs); undo check compares #END with its value before the
  cut (a song that already had #END).
- Still failing: copy 165, Alstroemeria Records - Bad Apple!! - `text editing: a double click
  on a lyric label did not open the inline field`. The song has out-of-order and stacked
  notes (`: 0 16 18`, `: 38 11 22`, `: 49 11 27`, `: 38 7 22`). The failure message now prints
  the click point, the beat, the note `noteAtLyricBeat` returns and the selection count:
  rerun that one song and read it. Suspects: `EditorController::note_at_lyric_beat` uses
  `take_while(start <= beat)` over notes in row order, which stops early on out-of-order
  notes (may be an app bug worth fixing test-first: pick the note with the largest start <=
  beat, or the label actually drawn there); or the click lands outside the lyrics lane.
- Then rerun the whole sweep until 136 of 136 pass, run `scripts/dev.sh check`, and replace
  the "Corpus sweep (open ...)" text in PROGRESS row R-04 with the final result.

### 3.2 Finish P7-01

`scripts/dev.sh check` (clippy `-D warnings`, rustfmt) with the new module; then mark P7-01
done in PROGRESS. The write-back of a `Placed` result into a patch belongs to P7-02.

### 3.3 P7-02 Anchor + stretch onward

User decision 2026-11-15 ("Retimed lines + neighbours"): a repair runs the collision pass only
over the retimed lines plus one line before and after them; overlaps elsewhere in the song
(Factory stacked notes, 23 notes in 6 corpus voices) stay untouched. Plan 6.7 to 6.11 and the
task list in plan section 10 (phase 7) define the rest; gate G-06 applies to P7-10 (threshold
tuning with the user). The Factory reference for the anchors export (P7-06) is in plan section
12 (`anchors.py`, `outro.py:parse_time`, `docs/repairs.md`).

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
  under the lines pane. i64 invokable arguments are `std::int64_t` (`long`), not `qint64`.
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
