# USDX Song Editor - Feature status

Where the application stands, task by task. The work plan is `Plan/IMPLEMENTATION_PLAN.md`,
the task checklist `Plan/PROGRESS.md`; this page translates them into user-visible terms.

Version 0.1.0, development build (13 November 2026).

## In the current build

| Feature | Where |
|---|---|
| Open songs losslessly: encodings (UTF-8 with or without BOM, CP1252, CP1250), line endings, relative mode, duets, unknown headers and note types | Open, drag and drop, command line |
| Refusal of non-songs with exact messages (`no #BPM header`, `could not be read`) | open flow |
| Read-only handling for format versions above 1.x and files that would not round trip | editor |
| Lines pane: every line of all voices, global numbering, type chips, search filter, click to jump | editor |
| Line navigation with Up/Down and clamping | editor |
| Line header: time span, duration, note count, syllable rate, health pill (always "timing ok" yet), line type selector | editor |
| Status bar: BPM, GAP, format version, unsaved / read-only / audio missing pills | editor |
| Start screen with the 10 most recent songs, relative dates, `~` paths, missing-file handling | start screen |
| Undo and redo with descriptive labels | toolbar, Ctrl+Z / Ctrl+Shift+Z |
| Save, Save As, first-save `.bak` backup, atomic write, external-change detection | save flow |
| Save changes? and Song changed on disk guards on close, open, reload and drop | dialogs |
| Cannot open song error dialog with the exact texts | dialogs |
| Window size persistence, activation check with silent reload on window focus | window |
| Main menu drawer with Open Recent submenu, Clear List, shortcuts | menu |
| Settings persistence (recent songs, window size, the full 4.10 field set) | settings file |
| About page with version and licence | menu |
| Offscreen self-test (`--self-test [SONG]`) that verifies the window and reports line count and title | command line |

The lossless file model underneath is complete and corpus-tested: 136 real songs from the
USDX Song Factory output round trip byte-identically.

## Not yet (later phases of the plan)

| Feature | Planned phase |
|---|---|
| Screenshot harness for acceptance tests (`--screenshot`, `--demo-state`) | P3-06 |
| Playback: player, playhead, play line, loop, track and speed controls act | P4 |
| Waveform and vocal activity lanes, audio analysis and cache | P4 |
| Note sheet: viewport, background, notes on pitch rows, ruler, lyrics lane | P5 |
| Note editing with mouse and keyboard: select, drag, resize, split, merge, insert, delete, type keys | P5 |
| Line text and syllable editing dialogs, inspector | P5 / P6 |
| Repair timing: anchor + stretch, block move and scale, tap-along | P7 |
| Timing health analysis (suspicious / racing marks with reasons) | P7 |
| Anchors export for the USDX Song Factory | P7 |
| Song properties page with validation list | P8 |
| Upgrade to format 1.1.0 | P8 |
| Settings page in the UI (the settings file already exists) | P8 |
| Packaging (PKGBUILD), documentation polish | P8 |

Until the note sheet lands, the toolbar's playback and repair controls show their final
enablement but do not act, and the note sheet area shows the view background.
