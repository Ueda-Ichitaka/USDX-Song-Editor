# USDX Song Editor - User manual

This manual describes the application as specified; sections note where a feature is not
part of the current development build yet (see `Docs/status.md` for the exact list).
Everything described here follows the binding specification in `Plan/UI_SPEC.md`.

- Application: USDX Song Editor (`io.github.ueda_ichitaka.UsdxSongEditor`)
- Version this manual describes: 0.1.0

## Contents

1. Starting and quitting
2. The start screen
3. Opening songs
4. The editor window
5. Saving and backups
6. Read-only songs
7. Audio states
8. Dialogs and messages
9. Main menu
10. Keyboard shortcuts
11. Settings
12. The About page
13. Where the settings live

## 1. Starting and quitting

Start the application from the KDE menu ("USDX Song Editor") or run `usdx-song-editor` in
a terminal. With a song path as argument the application opens straight into the editor:

```sh
usdx-song-editor /path/to/song.txt
```

The window opens at the size it had when you last closed it (1280 x 800 the first time);
the minimum is 960 x 600. The title bar always tells you where you are:

```
<Artist> - <Title> · USDX Song Editor
```

with a `* ` prefix while the song has unsaved changes and a ` (read-only)` suffix for
read-only songs. Without a song the title is just `USDX Song Editor`.

**Quitting:** Ctrl+Q or the main menu's Quit. If the song has unsaved changes the Save
changes? dialog asks first (section 8); the window size is remembered only when the window
really closes.

## 2. The start screen

Without a song the editor page shows a placeholder: "No song open" with the hint "Open an
UltraStar song file (.txt), or drop one onto this window" and an **Open song...** action.

Below it sits the recent songs list (up to 10 entries). Each row shows:

- first line: `Artist - Title`, read from the song file itself; a song without those
  headers is listed under its file name
- right of it: when you last opened it - "today", "yesterday", or the date
- second line: the folder path with `~` for your home folder

Missing files stay in the list but are greyed out and show "File not found". Click a row
to open the song. "Clear List" in the Open Recent submenu empties the list.

## 3. Opening songs

Four ways, all guarded the same way (section 8):

- **Open** in the toolbar or Open... in the main menu (Ctrl+O). The file dialog starts in
  the folder of the current song, else of the most recent song, else your home folder.
- A row in the recent songs list.
- **Drag and drop**: drop a `.txt` file anywhere on the window. Dropping something else
  shows "Only UltraStar song files (.txt) can be opened."
- The command line (section 1).

A file that cannot be read or is not a song shows the Cannot open song dialog (section 8)
and keeps the previously loaded song open.

## 4. The editor window

The editor page is a toolbar on top, the song area below and a status bar at the bottom.

### 4.1 Toolbar

Left to right:

| Control | Does |
|---|---|
| Main menu (icon) | opens the main menu drawer (section 9) |
| Open | opens the file dialog |
| Save | saves to the song's own file (enabled with unsaved changes on an editable song) |
| Undo / Redo | undo and redo of every edit step, unlimited |
| Play | plays the song from the current position (Space) |
| Play line | plays the current line with 0.3 s padding before and after (P) |
| Loop | repeats the current line or selection (L) |
| Track | picks Mix, Vocals or Instrumental; missing tracks are greyed out (V). A newly opened song starts on the track the settings choose (Vocals when the song has a vocals file) |
| Speed | 0.5x, 0.75x or 1x ([ and ]) |
| Repair timing | opens the repair mode (Ctrl+R) |
| Song properties | opens the song properties page |

Without a song the toolbar keeps only Play of the playback controls. In the current build
Play, Play line, Loop, Track and Speed act; Loop repeats the current line. Repair timing and
Song properties arrive with the later phases and show a notice until then (see
`Docs/status.md`).

### 4.2 The lines pane (left)

The pane lists every lyric line of all voices with:

- the global line number (1-based across all voices)
- the start time (m:ss.s)
- a type chip before the text for lines that are not all normal notes: `*` golden, `F`
  freestyle, `R` rap, `G` rap golden, "mixed" when the line mixes types
- the joined syllable text, elided when too long
- a health dot on the right: green "timing looks plausible", amber "suspicious", red
  "racing" with a red bar at the row's left edge; hovering shows the reason

The search field filters the rows case-insensitively by lyric text. The current line is
highlighted and the list follows it; clicking a row jumps to that line.

### 4.3 The line header (right, above the sheet)

Shows for the current line: its position ("Line 5 of 23"), the time span in m:ss.ss, the
duration, the note count, the syllable rate (syllables per second), the timing health as a
coloured pill, the line type selector and an **Edit text...** button for the line text
dialog.

### 4.4 The note sheet (right, centre)

The sheet shows the notes of the current line on their pitch rows, with the ruler, the
lyrics lane and the waveform lane, all drag- and keyboard-editable. This is the heart of
the editor and arrives with the note sheet phase.

The waveform lanes at the bottom of the sheet are already there. They show the current line
with at least 1.5 s around it (half the line's length for long lines), and stay put while you
step through lines that are already visible:

- the waveform of the selected track (the gutter names it: Mix, Vocals or Instr.), scaled so
  the loudest moment of the song fills the lane;
- below it a thin green lane where the `#VOCALS` track is singing.

While a song's audio is read the first time, the lane shows "Reading the waveform... N %";
later openings are instant because the result is cached. Without an audio file it says "No
audio", and without a vocals track a note explains that vocal activity is off.

### 4.5 Status bar (bottom)

The song facts on the left: **BPM** (two decimals), **GAP** (whole milliseconds),
**Playhead** (the playback position in m:ss.ss and its beat), **Format** (the `#VERSION`
value, or "unversioned"); without a song it says "No song open". On the right the state
pills:

- `N unsaved changes` - blue while the song differs from its file
- `read-only` - red; the song cannot be saved
- `audio missing` - grey; the named audio file is not in the song folder

## 5. Saving and backups

**Save** (Ctrl+S) writes the song to its own file, **Save As...** (Ctrl+Shift+S) writes to
a file you pick and switches the editor to it. A save:

1. refuses read-only songs (they show the read-only banner instead),
2. checks the file on disk: if another program changed it since you opened it, the Song
   changed on disk dialog asks first (section 8),
3. on the first save of a session copies the file as it is to `<song>.txt.bak` (the banner
   note says so; the notification mentions the backup),
4. writes atomically: the file is written to a temporary name and renamed over the
   original, so a crash mid-save cannot destroy your song,
5. clears the unsaved-changes state.

A failed save shows the save error banner above the sheet with the exact reason (for
example `Could not save: no permission to write "song.txt". Your changes are still here.`)
and **Try again** and **Save as...** actions. The banner stays until a save succeeds;
nothing is lost. There is no auto-save: the unsaved-changes pill and the `*` in the title
tell you what is not written yet.

## 6. Read-only songs

A song opens read-only when its `#VERSION` has a major version the editor cannot write
back losslessly (anything above 1), or when writing it back would change bytes you did not
edit. Read-only songs show:

- the red error banner above the sheet ("can show and play it, but cannot change or save
  it"),
- `read-only` in the status bar, ` (read-only)` in the window title,
- disabled editing: Save, Save As, Undo/Redo, Repair timing and all editing keys.

You can still navigate, play and export anchors from a read-only song.

## 7. Audio states

The toolbar's play controls depend on the audio files in the song folder:

| State | Banner | Toolbar |
|---|---|---|
| Audio file present | none | Play, Play line, Loop, Track, Speed enabled |
| Song names an audio file that is not in the folder | warning: "The audio file `<name>` is not in the song folder..." with a Song properties action | playback disabled |
| Song names no audio file at all | warning: "This song names no audio file..." | playback disabled |
| Song has an instrumental track | none | Instrumental track selectable |

The Mix track is the `#AUDIO` header value, falling back to `#MP3`; Vocals and
Instrumental come from `#VOCALS` and `#INSTRUMENTAL`.

## 8. Dialogs and messages

### Save changes? (type Warning)

Shown before closing the window, opening another song, opening a recent song, dropping a
file, or reloading, when the song has unsaved changes:

> "Song Title" has 2 unsaved changes. If you continue without saving, they are lost.

Buttons: **Save** (saves and then continues; a failed save stops the action and keeps the
banner), **Discard** (continues without saving), **Cancel** (stays where you were).
Esc is Cancel.

### Song changed on disk (type Warning)

Shown when a save hits an external change or the window regains focus and the file differs
from the last known state:

> Another program changed "song.txt" after you opened it.
> Overwrite replaces those changes with yours. Reload discards your 2 unsaved changes and
> opens the file as it is now.

Buttons: **Overwrite** (saves your version and continues), **Reload** (drops your changes
and opens the file as it is now), **Cancel**. Without unsaved changes the second sentence
is shorter.

### Cannot open song (type Error)

> "song.txt" is not an UltraStar song: it has no #BPM header.
> "song.txt" could not be read: <reason>.

Button OK. The previously loaded song stays open.

### Passive notifications (4 s, 2 s for "Saved")

- `Saved` and, on the first save of a session, `Saved. Backup written to "song.txt.bak".`
- `Reloaded: the file changed on disk.` (after a silent reload on window focus)
- `Only UltraStar song files (.txt) can be opened.` (wrong drop)
- `Notes cannot move before the song start. Lower #GAP in Song properties first.` and the
  other refused-edit texts (with the note sheet phase)

## 9. Main menu

The toolbar's leftmost button opens the menu drawer:

| Item | Shortcut | Notes |
|---|---|---|
| Open... | Ctrl+O | |
| Open Recent | | submenu with one entry per recent song plus Clear List |
| Save | Ctrl+S | enabled with unsaved changes on an editable song |
| Save As... | Ctrl+Shift+S | |
| Reload from Disk | | drops unsaved changes (asks first) and re-opens the file |
| Song Properties | | title, artist, BPM, GAP, START, END, audio file |
| Repair Timing | Ctrl+R | |
| Export Anchors for the Factory... | | writes `anchors.txt` in the Factory's format |
| Settings | Ctrl+, | |
| About USDX Song Editor | | name, version, licence, repository |
| Quit | Ctrl+Q | |

## 10. Keyboard shortcuts

The editor's own shortcuts (menu shortcuts in section 9; Esc closes a dialog, leaves a
pushed page, stops playback, or clears the selection):

| Key | Action |
|---|---|
| Space | play / pause |
| P / Shift+P | play line / play selection |
| L | loop line on/off |
| V | next track (skips missing ones) |
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
| Ctrl+Z / Ctrl+Shift+Z | undo / redo |

In the current build only line navigation (Up/Down and clicking), undo/redo, Save, the
menu shortcuts and the window actions are active; the rest arrive with the note sheet and
playback phases.

## 11. Settings

The Settings page (Ctrl+,) has the sections Playback, Tap-along, Timing health and Saving.
Everything applies and is saved at once; **Restore defaults** asks first and keeps the
recent songs and the window size. Defaults:

| Setting | Default |
|---|---|
| Track when a song opens | Vocals if present, else Mix |
| Time added before and after a played line | 0.3 s |
| Play each line you move to | off |
| Tap latency | 35 ms (with a Calibrate... dialog) |
| Pre-roll before the first line | 2.0 s |
| Speed when tap-along starts | 0.5x |
| Suspicious above | 7.0 syllables per second |
| Racing above | 10.0 syllables per second |
| Suspicious when faster than neighbours by | 2.5 times |
| Suspicious when vocals are heard less than | 40 % of the note time |
| Racing when vocals are heard less than | 20 % of the note time |
| Vocals count as heard above the noise floor by | 12 dB |

The Settings page itself arrives with the settings phase; until then the settings file
(section 13) already backs the parts the current build uses.

## 12. The About page

"About USDX Song Editor" in the main menu pushes the About page with the application name,
version, the MIT licence and the repository URL. The **Editor** button (or Esc) returns.

## 13. Where the settings live

`~/.config/io.github.ueda_ichitaka.UsdxSongEditor/settings.toml`. It is read at start (a
missing or broken file gives the defaults with a warning in the log) and written on every
change. The recent songs list and the window size live there too.
