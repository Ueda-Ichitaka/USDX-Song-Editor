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
| Playback: Play, Play line (0.3 s padding), Loop of the current line, Track (Mix, Vocals, Instrumental, chosen per song from the settings), Speed (0.5x, 0.75x, 1x, pitch kept), playhead time and beat in the status bar | toolbar, Space / P / L / V / [ / ] |
| Audio analysis (waveform peaks and vocal activity) with an on-disk cache, used by the waveform lane once it is drawn | background |
| Offscreen self-test (`--self-test [SONG]`) that opens the song through the window's open guard, drives the dialogs, the recent menu and quit, and fails on any Qt warning | command line |
| Screenshot harness (`--screenshot`, `--demo-state`, debug builds) for the UI acceptance check | command line, `scripts/ui-shots.sh` |

The lossless file model underneath is complete and corpus-tested: 136 real songs from the
USDX Song Factory output round trip byte-identically.

## Not yet (later phases of the plan)

| Feature | Planned phase |
|---|---|
| Waveform and vocal activity lanes drawn under the sheet, audio-state screens | P4 (P4-04, in progress) |
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

Until the note sheet lands, the repair and song properties controls show their final
enablement but do not act, and the note sheet area shows the view background.
