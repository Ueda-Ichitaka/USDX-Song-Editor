<!--
About me: the binding user interface specification of the USDX Song Editor. It fixes, for
every screen, state and dialog, the component tree, sizes, colours, fonts, texts and
behaviour precisely enough that any agent on any model builds the same UI. The reference
images it points to are in Plan/mockups/ (rendered from Plan/usdx-song-editor-plan.html by
Plan/render_mockups.sh). Plan/IMPLEMENTATION_PLAN.md says when each part is built.
-->

# USDX Song Editor - UI Specification

Version 1, 12 November 2026. Companion to `Plan/IMPLEMENTATION_PLAN.md` (the build plan) and
`Plan/usdx-song-editor-plan.html` (the interactive mock page).

## Contents

1. How to use this specification
2. Global rules: units, fonts, colours, icons, texts, window
3. The note sheet (drawing rules)
4. Screens
5. Dialogs
6. Messages and notifications
7. Enablement matrix
8. Screenshot harness and acceptance procedure

---

## 1. How to use this specification

- **Binding.** Every UI task in the build plan must produce what this document says. The
  reference images in `Plan/mockups/` show the intended result for each screen in both
  themes. When text and image disagree, this text wins. When this text is silent, the image
  wins. When both are silent, ask the user (gate G-07 of the build plan).
- **Native controls win on their own look.** The mock page imitates Breeze in HTML. Where a
  Kirigami or Qt Quick Controls component draws itself (buttons, combo boxes, spin boxes,
  text fields, menus, dialogs, `PlaceholderMessage`, `InlineMessage`), use the component
  unstyled; its own padding, height and font are correct even if they differ by a few
  pixels from the mock. Everything the app draws itself (note sheet, lanes, list rows,
  chips, pills, health dots, anchor flags, tap key) must follow the numbers in this document.
- **Reference images**: `Plan/mockups/<name>-light.png` and `<name>-dark.png`. Names:

| Name | Screen | Section |
|---|---|---|
| `start-screen` | No song open | 4.1 |
| `main-menu` | Main menu with Open Recent | 4.2 |
| `editor` | Editor | 4.3 |
| `audio-states` | Missing audio, waveform loading, no vocals track | 4.4 |
| `read-only` | Read-only song | 4.5 |
| `line-text` | Line text dialog | 5.4 |
| `repair-anchor` | Repair: anchor + stretch | 4.6 |
| `repair-block` | Repair: block move/scale | 4.7 |
| `tap-along` | Tap-along | 4.8 |
| `song-properties` | Song properties | 4.9 |
| `settings` | Settings | 4.10 |
| `unsaved-changes` | Save changes? | 5.1 |
| `changed-on-disk` | Song changed on disk | 5.2 |
| `export-anchors` | Export anchors | 5.5 |
| `upgrade` | Upgrade to format 1.1.0 | 5.6 |
| `latency-calibration` | Measure tap latency | 5.7 |
| `errors-and-notifications` | Save error, refused action, open error | 6 |

- To regenerate the images after changing the mock page: `Plan/render_mockups.sh`
  (needs Firefox and ImageMagick; checks memory before each run).

---

## 2. Global rules

### 2.1 Units

Measured on this machine with Kirigami 6.31 and the default font (12 November 2026):

| Kirigami value | px |
|---|---|
| `Kirigami.Units.gridUnit` | 18 |
| `Kirigami.Units.smallSpacing` / `mediumSpacing` / `largeSpacing` | 4 / 6 / 8 |
| `Kirigami.Units.cornerRadius` | 5 |
| `Kirigami.Units.iconSizes.small` / `smallMedium` / `medium` / `large` / `huge` | 16 / 22 / 32 / 48 / 64 |

Sizes in this document are given in px at that scale. In QML, write them in Kirigami units
where the value is a clean multiple (for example `Kirigami.Units.gridUnit * 16` for 288 px);
otherwise as a named constant in `qml/Metrics.qml` (a singleton, one place for every app
metric in this document). Never scatter literal pixel numbers in other QML files.

### 2.2 Fonts

| Use | Font |
|---|---|
| Body, controls | `Kirigami.Theme.defaultFont` (Noto Sans 10 pt on this machine) |
| Secondary text, list numbers, ruler, gutter labels, status bar, captions | `Kirigami.Theme.smallFont` (8 pt) |
| Times, beats, numbers in columns, file preview, diff | `Kirigami.Theme.fixedWidthFont` (Hack), `pointSize` 9 |
| Dialog titles | `defaultFont`, `pointSize` 11, `Font.DemiBold` (use the dialog's own `title` property; it already renders this way) |
| Line header "Line 3 of 47" | `defaultFont`, `Font.DemiBold` |
| Syllables of the current line in the lyrics lane | `defaultFont`, `pointSize` 9, `Font.DemiBold` |
| Tap-along current syllables | `defaultFont`, `pointSize` 22, `Font.DemiBold` |
| Section labels in panels ("ANCHORS", "CHECKS") | `smallFont`, `Font.Bold`, upper case via `font.capitalization: Font.AllUppercase`, letter spacing 0.5 px, `disabledTextColor` |

### 2.3 Colours

All UI colours come from `Kirigami.Theme` roles so both themes work. The hex values are the
Breeze Light / Breeze Dark values from `/usr/share/color-schemes/` for reference and for
checking screenshots.

| Role in this spec | Kirigami | Breeze Light | Breeze Dark |
|---|---|---|---|
| window background | `Theme.backgroundColor` in `colorSet: Window` | `#eff0f1` | `#202326` |
| view background (lists, sheet) | `Theme.backgroundColor` in `colorSet: View` | `#ffffff` | `#141618` |
| alternate row | `Theme.alternateBackgroundColor` (View) | `#f7f7f7` | `#1d1f22` |
| header / ruler background | `colorSet: Header`, `backgroundColor` | `#dee0e2` | `#292c30` |
| text | `Theme.textColor` | `#232629` | `#fcfcfc` |
| secondary text | `Theme.disabledTextColor` | `#707d8a` | `#a1a9b1` |
| highlight, selection | `Theme.highlightColor` | `#3daee9` | `#3daee9` |
| text on highlight | `Theme.highlightedTextColor` | `#ffffff` | `#fcfcfc` |
| positive (health ok, vocal activity) | `Theme.positiveTextColor` | `#27ae60` | `#27ae60` |
| neutral (suspicious, warnings) | `Theme.neutralTextColor` | `#f67400` | `#f67400` |
| negative (racing, errors, playhead) | `Theme.negativeTextColor` | `#da4453` | `#da4453` |
| positive / neutral / negative background | `Theme.positiveBackgroundColor` etc. | (theme) | `#0d2c1b` / `#3c1f05` / `#3c161a` |

Derived colours, computed with `Kirigami.ColorUtils.linearInterpolation(a, b, f)` (verified
present, as are `tintWithAlpha`, `adjustColor`, `brightnessForColor`):

| Name | Formula |
|---|---|
| grid line | `linearInterpolation(viewBackground, text, 0.10)` |
| strong grid line (C rows) | `linearInterpolation(viewBackground, text, 0.20)` |
| dialog backdrop | black at 45 % (light) / 55 % (dark) opacity; use the Kirigami dialog's own backdrop if it has one |

App colours (not theme roles), defined once in `qml/Palette.qml` (singleton). "Dark" means
`Kirigami.ColorUtils.brightnessForColor(Kirigami.Theme.backgroundColor) === Kirigami.ColorUtils.Dark`.

| Name | Light | Dark | Used for |
|---|---|---|---|
| note normal `:` | `#3daee9` | `#3daee9` | note fill, type chip |
| note golden `*` | `#f0b429` | `#f0b429` | |
| note freestyle `F` | `#b8bec7` | `#7d848e` | |
| note rap `R` | `#2fb6a6` | `#2fb6a6` | |
| note rap golden `G` | `#9b7fe6` | `#9b7fe6` | |
| note other (unknown type) | freestyle colour with a 1 px `neutralTextColor` border | same | |
| chip text | `#10151c` | `#0b0e12` | text on type chips |
| anchor | `#a3245e` | `#f27fb3` | anchor flags, anchor lines, anchor badges (white text on light, `#0b0e12` on dark) |
| waveform | `highlightColor` at 55 % opacity | same | |
| vocal activity | `positiveTextColor` at 55 % opacity | same | |
| ghost note outline | `disabledTextColor`, 1.2 px, dash 3/3 | same | retime preview |
| selection band | `highlightColor` at 12 % opacity, edges 2 px `highlightColor` | same | block selection |

### 2.4 Icons

Breeze icon names (all verified present in `/usr/share/icons/breeze` on 12 November 2026):

| Action | Icon |
|---|---|
| Main menu | `application-menu` |
| Open, Open song… | `document-open` |
| Open Recent | `document-open-recent` |
| Save / Save As | `document-save` / `document-save-as` |
| Reload from Disk, Start again, Restore defaults | `view-refresh` |
| Undo / Redo | `edit-undo` / `edit-redo` |
| Play / Pause / Stop | `media-playback-start` / `media-playback-pause` / `media-playback-stop` |
| Play line | `view-media-lyrics` |
| Loop | `media-playlist-repeat` |
| Repair timing | `tools-wizard` |
| Song properties | `document-properties` |
| Edit text | `document-edit` |
| Back, Editor (back) | `go-previous` |
| Pin line start here | `pin` |
| Preview | `view-visible` |
| Export, Export anchors | `document-export` |
| Upgrade | `go-up` |
| Settings | `settings-configure` |
| About | `help-about` |
| Quit | `application-exit` |
| Calibrate | `input-keyboard` |
| Discard, Clear List | `edit-delete` |
| Change… (folder) | `folder-open` |
| Song in recent lists, start screen | `audio-x-generic` |
| Warning / error / info in dialogs | provided by `PromptDialog.dialogType` |

### 2.5 Texts

- Every visible string goes through `qsTr()` (decision D-10). The strings in this document
  and in the reference images are the exact English texts. Quotation marks are straight
  ASCII `"`. The ellipsis character `…` (U+2026) ends labels of actions that open a dialog.
- Menu items use title case ("Open Recent", "Save As…"). Everything else uses sentence case
  ("Play line", "Repair timing", "Song properties").
- Numbers: times in the line list `m:ss.s`; times everywhere else `m:ss.ss`; durations
  `4.8 s`; rates `2.5 syl/s`; beats as integers; pitches as number plus note name (`7` `G4`,
  note names with `#`, octave 4 starting at C4 = pitch 0); speeds `0.5x`, `0.75x`, `1x`.
  A dot as the decimal separator regardless of locale (these are file values).
- Separators inside one label: ` · ` (space, U+00B7 middle dot, space). Ranges: `0:21.48 - 0:26.27`
  (spaced hyphen-minus). Never use the en dash.

### 2.6 Window

- `Kirigami.ApplicationWindow`, default size 1280 x 800, minimum 960 x 600. The size is
  remembered in the settings.
- Title: `"<Title> - <Artist> · USDX Song Editor"` from `#TITLE` and `#ARTIST`. Prefix
  `"* "` while there are unsaved changes. Suffix `" (read-only)"` after the artist for a
  read-only song. Without a song: `"USDX Song Editor"`. Sub-pages: `"Repair timing - <Title> · USDX Song Editor"`,
  `"Song properties - <Title> · USDX Song Editor"`, `"Settings · USDX Song Editor"`.
- `pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None`. Every page draws its
  own toolbar (section 4) as a `QQC2.ToolBar` in its `header`, so toolbar contents and order
  are fully under our control.
- Pages are pushed onto `pageStack`: Editor is the initial page; Repair timing, Song
  properties and Settings are pushed on top and return with their Back/Editor button or Esc.
- The app opens on the start screen (4.1) or, with a file argument, straight into the editor.

---

## 3. The note sheet

The sheet is the centre of the editor (`editor` image) and, in variants, of the repair
screens. It is one `Flickable` containing, from back to front: `SheetBackground` (a Rust
`QQuickPaintedItem`, paints everything that is not interactive), the note delegates, the
phrase end handles, the rubber band, and the playhead.

### 3.1 Vertical structure (top to bottom)

| Part | Height | Background | Content |
|---|---|---|---|
| Ruler | 22 px | header background | time labels and ticks |
| Pitch rows | the rest (see 3.3) | view / alternate | notes, phrase ends |
| Lyrics lane | 34 px | window background | syllables (3.6) |
| Gap | 8 px | view background | |
| Waveform lane | 54 px | window background | waveform (3.7) |
| Activity lane | 6 px | view background | vocal activity (3.7) |

A left gutter of 46 px runs the full height: pitch labels in the pitch rows, the words
"Lyrics" and the track name ("Mix", "Vocals", "Instr.") in the lanes (`smallFont`,
`disabledTextColor`, left aligned at 4 px, vertically centred). In the repair sheets the
lyrics lane is left out.

### 3.2 Horizontal scale and default view

- The x axis is beats. `px_per_beat` is clamped to 1.0 to 40.0.
- When a line becomes current, the view shows the line's first note start to last note end,
  plus padding on both sides of `max(1.5 s, 50 % of the line's duration)`, converted to
  beats with the song timing. The view only scrolls when the new line is not fully visible
  (stepping through short neighbouring lines does not jump).
- Ctrl+wheel zooms by a factor of 1.25 per wheel step around the beat under the cursor.
  Wheel scrolls time by 10 % of the visible width per step. Shift+wheel scrolls the pitch
  range by one row per step. Middle-drag pans.

### 3.3 Pitch rows

- Visible pitch range: lowest to highest pitch of the current line, extended by 3 semitones
  on both sides, then extended symmetrically to at least 14 rows.
- Row height = pitch area height / rows, at least 14 px. If rows do not fit at 14 px, the
  pitch area scrolls vertically.
- Row background: alternate row colour for C and for the black keys (C#, D#, F#, G#, A#),
  view background for the others. A 1 px grid line under each row; the line under a C row is
  the strong grid line.
- Pitch labels (gutter, right aligned, 6 px from the gutter's right edge, vertically centred
  in the row): every row when row height is at least 18 px, otherwise only C rows and even
  pitches. C rows in `Font.DemiBold` and `textColor`, others `smallFont` and `disabledTextColor`.
  Format: note name with `#` and octave (`C4`, `F#4`, `C5`).

### 3.4 Ruler

- Labels `m:ss.s`, `smallFont`, `disabledTextColor`, 3 px right of their tick, baseline at
  14 px.
- Tick interval: the smallest of 0.5, 1, 2, 5, 10, 30, 60 s that keeps ticks at least 70 px
  apart. Each tick draws a 1 px grid line from 6 px above the ruler's bottom through the
  pitch rows.
- A click or drag in the ruler moves the playhead (seek).

### 3.5 Notes and markers

| Element | Drawing |
|---|---|
| Note | Rectangle from `x(start)` to `x(start + length) - 1.5 px`, top = row top + 2 px, height = row height - 4 px, radius 3 px, fill = note type colour. At least 2 px wide |
| Notes of the previous and next line | Same, opacity 0.35. A click makes their line current and selects the note |
| Selected note | 2 px border in `textColor`; two edge handles 4 x 12 px, radius 1, `textColor`, centred on the left and right edge (only when the note is at least 12 px wide) |
| Hover | Cursor `Qt.SizeAllCursor` on the body, `Qt.SizeHorCursor` within 5 px of an edge |
| Dragged note | Moves with the pointer; no other visual change. Commit on release (build plan 8.4) |
| Phrase end | Dashed vertical line, 1.4 px, dash 5 / gap 4, `disabledTextColor`, through the pitch rows; an 8 x 8 px square cap with radius 2 at the top of the pitch rows (2 px below the ruler). Cursor `Qt.SizeHorCursor` on the cap and line |
| Playhead | 2 px line in `negativeTextColor` from the top of the ruler to the bottom of the activity lane; a downward triangle 12 px wide, 8 px high at the top of the ruler |
| Rubber band | `highlightColor` at 15 % opacity fill, 1 px `highlightColor` border |
| Ghost note (preview) | Ghost outline (2.3) at the preview position, drawn above the notes |
| Notes of a racing line in the repair sheets | Fill `negativeTextColor` instead of the type colour |

### 3.6 Lyrics lane

- One label per note of the current line and of the faded neighbour lines, at `x(start)`,
  text = the note's text trimmed (a `~` is shown as `~`).
- Two text rows, baselines at 14 px and 29 px from the lane top. Placement: walk the notes in
  order; put the label in row 1 if it starts after the end of the last label in row 1,
  otherwise in row 2 if it starts after the last label in row 2, otherwise in row 1. A label
  that would still overlap is elided on the right (`…`) at the next label's start.
- Current line: `pointSize` 9, `Font.DemiBold`, `textColor`. Neighbour lines: `pointSize` 9,
  normal weight, `disabledTextColor` at 70 % opacity.
- Double-clicking a label edits that note's text (same as double-clicking the note).

### 3.7 Waveform and activity lanes

- Waveform: the min/max envelope of the selected track for the visible time range, filled,
  centred vertically, scaled so the loudest bucket of the whole song uses 92 % of the lane
  height. Colour: waveform (2.3).
- Activity: 6 px lane, `vocal activity` colour where the `#VOCALS` track is active, nothing
  elsewhere. Without a vocals track the lane is empty and the waveform lane shows the note
  "No vocals track: vocal activity and the coverage check are off. Vocals is disabled in the
  track list." centred in `smallFont`, `disabledTextColor` (`audio-states` image, case 3).
- While analysis runs: the lane shows "Reading the waveform… 42 %" centred, with a 260 x 6 px
  progress bar under it (`QQC2.ProgressBar`, `audio-states` case 1).
- Without audio: "No audio" centred (`audio-states` case 2).

---

## 4. Screens

Toolbars below list items left to right. `|` is a `Kirigami.Separator` (vertical, 1 px).
`<->` is a flexible spacer. All toolbar buttons are `QQC2.ToolButton` with
`display: AbstractButton.TextBesideIcon` unless "icon only" is stated. Checkable buttons show
their checked state natively.

### 4.1 Start screen (`start-screen`)

- Shown inside the editor page when no song is open.
- Toolbar: `[Main menu, icon only] [Open] [Save, disabled] | [Undo, disabled] [Redo, disabled] | [Play, disabled] <-> [Repair timing, disabled] [Song properties, disabled]`.
- Body: view background. A `Kirigami.PlaceholderMessage` centred horizontally, its top at
  70 px: `icon.name: "audio-x-generic"`, `text: "No song open"`,
  `explanation: "Open an UltraStar song file (.txt), or drop one onto this window."`,
  `helpfulAction`: "Open song…" (`document-open`).
- Under it, 18 px gap, a 460 px wide list "Recent songs" (section label style), up to 10 rows.
  Row: first line "Artist - Title" (`defaultFont`) with the relative date right aligned
  (`smallFont`, `disabledTextColor`: "today", "yesterday", otherwise "10 Nov 2026"); second
  line the folder path with `~` for the home folder, `smallFont`, `disabledTextColor`, one
  line, elided in the middle. 1 px grid line between rows. Click opens the song. A song whose
  file no longer exists shows "File not found" as its second line in `negativeTextColor` and
  is not clickable; it is removed from the list on the next start.
- Without recent songs the list and its label are hidden.
- Status bar: "No song open".
- Dropping a `.txt` file on the window opens it (unsaved-changes dialog first if needed).
  Dropping anything else: passive notification "Only UltraStar song files (.txt) can be opened."

### 4.2 Main menu (`main-menu`)

- `globalDrawer: Kirigami.GlobalDrawer { isMenu: true }`, opened by the toolbar's Main menu
  button. Verify in task P3-05 that an `isMenu` drawer can be opened from our own toolbar
  button; if it cannot, stop at gate G-08 (build plan) before using any other component.
- Items (icon, text, shortcut):

| Item | Shortcut | Enabled when |
|---|---|---|
| Open… | Ctrl+O | always |
| Open Recent (submenu: one item per recent song, "Artist - Title", icon `audio-x-generic`; separator; "Clear List") | | recent list not empty |
| Save | Ctrl+S | song open, unsaved changes, not read-only |
| Save As… | Ctrl+Shift+S | song open, not read-only |
| Reload from Disk | | song open |
| separator | | |
| Song Properties | | song open |
| Repair Timing | Ctrl+R | song open, not read-only |
| Export Anchors for the Factory… | | song open |
| separator | | |
| Settings | Ctrl+, | always |
| About USDX Song Editor | | always (opens `Kirigami.AboutPage` with name, version, licence MIT, the repository URL) |
| Quit | Ctrl+Q | always |

### 4.3 Editor (`editor`)

Layout (top to bottom): toolbar, then a `QQC2.SplitView` (horizontal) with the lines pane
and the editor column, then the status bar.

**Toolbar**:
`[Main menu, icon only] [Open] [Save] | [Undo] [Redo] | [Play] [Play line] [Loop] "Track" [ComboBox: Mix, Vocals, Instrumental] "Speed" [ComboBox: 0.5x, 0.75x, 1x] <-> [Repair timing] [Song properties]`

- Play is checkable: unchecked shows "Play" with `media-playback-start`; checked shows
  "Pause" with `media-playback-pause`. Loop is checkable.
- "Track" and "Speed" are `QQC2.Label`s in `disabledTextColor`, `smallFont`. The track combo
  lists only existing tracks enabled; missing ones are listed but disabled.
- Undo/Redo tooltips: "Undo <label>" / "Redo <label>" with the history label (for example
  "Undo Move 3 notes").

**Lines pane** (left side of the SplitView): default width 300 px, minimum 220, maximum 480,
remembered in the settings.
- Top: `Kirigami.SearchField`, placeholder "Search lyrics…", 8 px margin. Typing filters the
  list to lines containing the text (case-insensitive); the current line stays selected if it
  matches.
- `ListView` with `clip: true`, rows of 29 px:

| Column | Width | Content |
|---|---|---|
| number | 22 px, right aligned | line number from 1, `smallFont`, `disabledTextColor` |
| time | 44 px, 6 px left gap | start `m:ss.s`, `fixedWidthFont` at `smallFont` size, `disabledTextColor` |
| text | rest, 6 px left gap | optional chips, then the line text, elided right |
| health | 16 px | 9 px circle: positive / neutral / negative colour |

- Chips before the text (`smallFont` bold, chip text colour, 0 / 4 px padding, radius 3):
  the type chip when the line is not all normal ("*", "F", "R", "G" on the type colour, or
  "mixed" on the golden colour); the anchor chip "A1", "A2"... on the anchor colour, only on
  the repair screens.
- Even rows use the alternate row colour. The current line uses `highlightColor` with
  `highlightedTextColor` for all its text. A racing line has a 3 px bar in
  `negativeTextColor` on its left edge (repair screens and editor alike).
- Tooltip on the health dot: the health reason ("Racing: 11.9 syllables per second",
  "Suspicious: vocals heard during 34 % of the note time", "Timing looks plausible").

**Editor column**:

1. *Line header* (window background, 1 px border below, 8 / 10 px padding, items centred
   vertically, one line, nothing wraps):
   "Line 3 of 47" (DemiBold), then "0:21.48 - 0:26.27 · 4.8 s · 12 notes · 2.5 syl/s"
   (`fixedWidthFont` 9 pt, `disabledTextColor`), then a health pill ("timing ok" /
   "suspicious" / "racing · 11.9 syl/s"; `smallFont` bold, text in positive/neutral/negative
   colour on the matching background colour, fully rounded, 1 / 7 px padding), then `<->`,
   then "Line type" (`smallFont`, `disabledTextColor`) and a ComboBox with
   "Normal", "Golden", "Freestyle", "Rap", "Rap golden" (showing "Mixed", not selectable,
   when the notes differ), then a button "Edit text…" (`document-edit`). The header says
   "Whole line selected" in place of the times while the whole line is selected (Left past
   the first note).
2. *Note sheet* (section 3), filling the remaining height.
3. *Inspector* (window background, 1 px border above, 8 / 10 px padding, a `Flow` with 12 px
   spacing, groups never split): "Note 3 · "free "" (label `disabledTextColor`, the text in
   DemiBold with spaces shown as `␣` U+2423); "Type" + a row of five checkable buttons
   ":", "*", "F", "R", "G" (tooltips "Normal", "Golden", "Freestyle", "Rap", "Rap golden");
   "Start" + `QQC2.SpinBox` (beats) + the time `0:21.93` (`fixedWidthFont`); "Length" +
   SpinBox (min 1); "Pitch" + SpinBox + the note name ("G4"); "Text" + `QQC2.TextField`
   (min 90 px wide). Group labels are `smallFont`, `disabledTextColor`.
   - Several notes selected: "3 notes selected"; Type shows the common type or none checked;
     Start, Length and Text disabled; Pitch shows the common value or empty and applies a
     transposition when changed.
   - Nothing selected: "No note selected", every field disabled and empty.
   - Pitch is disabled (greyed, value kept) for F, R and G notes.

**Status bar** (window background, 1 px border above, 24 px high, 10 px side padding,
16 px spacing, `smallFont`, labels `disabledTextColor` with values in `textColor` DemiBold):
"BPM 369.14", "GAP 14937 ms", "Playhead 0:22.31 · beat 181", "Format 1.2.0" ("Format
unversioned" when there is no `#VERSION`), `<->`, then pills (same style as the health pill):
"2 unsaved changes" / "1 unsaved change" (highlight colour on `highlightColor` at 20 %),
"read-only" (negative), "audio missing" (neutral).

### 4.4 Audio states (`audio-states`)

- Audio file missing (the `#AUDIO`/`#MP3` header is empty or the file is not in the folder):
  a `Kirigami.InlineMessage` of type Warning, `position: Kirigami.InlineMessage.Header`,
  across the top of the editor column, text
  `The audio file "<name>" is not in the song folder. You can edit the song, but playback and the waveform are off.`
  (without a header: `This song names no audio file. You can edit the song, but playback and the waveform are off.`),
  action "Song properties". Play, Play line, Loop, the track combo and P, Space, L, V are
  disabled; status bar pill "audio missing".
- Waveform still being read and no vocals track: section 3.7.

### 4.5 Read-only song (`read-only`)

- Applies to a `#VERSION` with major version other than 1 and to files whose round trip is
  not exact (the serializer self-check fails).
- InlineMessage of type Error at the top of the editor column:
  `This song uses format version 2.0.0. The editor can show and play it, but cannot change or save it.`
  (for a failed round trip: `This file cannot be saved back without changing parts you did not edit. The editor shows it read-only.`).
- Disabled: Save, Undo, Redo, Repair timing, every editing shortcut, the line type combo,
  Edit text, all inspector fields, all drag handlers. Enabled: navigation, selection,
  playback, Song properties (read-only form), Export anchors.
- Title suffix " (read-only)"; status bar pill "read-only".

### 4.6 Repair: anchor + stretch (`repair-anchor`)

A page pushed from the editor (Ctrl+R). Layout: toolbar, overview strip, three columns
(lines pane 260 px, sheet, side panel 280 px), status bar.

**Toolbar**: `[Editor (go-previous)] | [mode buttons: "Anchor + stretch" "Block move/scale" "Tap-along", exclusive checkable] | [Play] [Pin line start here (pin)] "Track" [ComboBox] <-> [Export anchors.txt… (document-export)]`

**Overview strip** (view background, 8 / 10 px padding, 1 px border below): caption
"Whole song: one block per line, the vocal activity of the #VOCALS track below"
(`smallFont`, `disabledTextColor`), then a 64 px drawing over the whole song duration:
line blocks 20 px high at y 8, radius 2, coloured by health; a racing section underlaid with
`negativeTextColor` at 10 % from 2 px before to 2 px after it, full height; the activity
lane 8 px at y 34; anchor flags (anchor colour, 20 x 12 px, radius 2, white bold "A1" in
`smallFont`) with a 1.6 px anchor line from y 4 to y 46; time labels every 30 s at the
bottom (`smallFont`, `disabledTextColor`). Clicking the strip makes the line under the
pointer current and scrolls the sheet there. Implemented as `SheetBackground` in overview
mode.

**Lines pane**: as in the editor (4.3) at 260 px, with anchor chips and racing bars.

**Sheet column**: line header "Lines 15 - 22" + racing pill + "vocals heard during 17 % of the
note time" + `<->` + checkable "Preview" (`view-visible`, checked by default). Sheet as in
section 3 without the lyrics lane, notes of racing lines in `negativeTextColor`, ghost notes
for the preview, anchor lines and flags (22 x 14 px) at the anchor times through the full
height. Under the sheet a caption line: "Solid red: the notes as they are now. Dashed: where
they land after Apply. Flags: anchors." (`smallFont`, `disabledTextColor`).

**Side panel** (view background, 10 px padding, 10 px spacing):
1. Warning InlineMessage: "Lines 15-22 run 3.9 times faster than the lines around them. Pin
   the true start of the first line and of the first line after the section, then Apply."
   (shown while a racing section is selected and fewer than 2 anchors exist; otherwise
   hidden).
2. Section label "ANCHORS", then one card per anchor (1 px border, radius 4, 6 / 8 px
   padding): badge "A1" (anchor colour) left; "Line 15 Weakness kept in silence" (line
   number DemiBold, text elided); second row `1:06.18 → 1:06.18` in `fixedWidthFont` with
   "(unchanged)" or the offset "(+15.20 s)". Right-click or a small remove button deletes
   it. Last, a dashed card with a "+" badge (`disabledTextColor`): "Play, then press A at a
   line start" / "or drag a line onto the waveform".
3. Section label "BETWEEN ANCHORS", then a form: "Note lengths" [Scale | Keep] (exclusive
   buttons), "Stretch factor" `3.90x (lines 15-22)` (`fixedWidthFont`), "Collisions" pill
   "none after rounding" (positive) or "3 notes shortened" (neutral).
4. `<->`, then right-aligned buttons "Cancel" and "Apply stretch" (default button).

**Status bar**: "Section 1:06.18 - 1:12.00 becomes 1:06.18 - 1:28.89", "Apply is one undo step",
`<->`, pill "preview" (highlight) while the preview is on.

### 4.7 Repair: block move/scale (`repair-block`)

Same page, mode "Block move/scale". No lines pane and no side panel: the sheet uses the full
width; the lines are selected in the editor before switching, or by dragging on the sheet.

- Toolbar: `[Editor] | [mode buttons] | [Play selection] <-> "Snap" [Beat | Off]`.
- Sheet (no lyrics lane, no waveform lanes): the selected block as a selection band with
  2 px edges and two handles 8 x 32 px, radius 2, `textColor`, centred vertically on the
  edges. While an edge is dragged: a 2 px `highlightColor` arrow from the old to the new edge
  position with the label "drag edge: 1:12.00 → 1:28.89" (`defaultFont` DemiBold) above it,
  the old notes as ghosts, the new positions solid.
- Bar under the sheet (inspector style): "Selection" + "Lines 15 - 22 · 57 notes" (DemiBold);
  "Start" field; "End" field → new end field (highlighted border); "Scale" field "3.90x";
  "Following lines" [Stay | Move along]; `<->`; "Apply".
- Status bar: "Drag the block body to move it, drag an edge to scale it from the opposite
  edge. Hold Shift to turn snapping off."

### 4.8 Tap-along (`tap-along`)

Opened with T or the mode button; a page of its own, 760 px wide content centred in the
window.

- Toolbar: `[Repair (go-previous)] | "Tap" [Line starts | Syllables] "Speed" [0.5x | 0.75x | 1x] <-> "Latency" [field "35 ms", read-only] [Calibrate… (input-keyboard)]`.
- Main area (view background, 26 / 18 px padding, centred column, 14 px spacing):
  1. Status line `smallFont`, `disabledTextColor`: "Line 16 of 15-22 · pre-roll 2 s before line 15 · recording"
     (states: "ready - press Space to start", "pre-roll", "recording", "finished").
  2. The current line in 22 pt DemiBold: syllables already tapped in `disabledTextColor`,
     the next syllable to tap on `highlightColor` with `highlightedTextColor`, radius 4,
     0 / 6 px padding, the rest in `textColor`.
  3. "next: Throughout this endless crisis" in 12 pt `disabledTextColor`.
  4. A key cap 140 x 64 px, radius 10, 2 px border (5 px at the bottom) in the strong grid
     colour, window background, "Space" in 14 pt bold; it turns `highlightColor` for 80 ms on
     every tap.
  5. Help line `smallFont`, `disabledTextColor`: "Press Space at the start of each highlighted
     syllable. Hold it for the note length if you like. Backspace takes the last tap back."
- Lane (1 px border above, 10 px padding, 70 px): label "now" with the current notes as
  6 x 14 px blocks in `negativeTextColor`, label "taps" with each tap as a 2 px vertical
  tick in `textColor`, time labels every 2 s.
- Status bar: "Taps 12 / 57", "Playback 0.5x, pitch kept", `<->`, [Stop], [Use taps]
  (default button, enabled when at least one tap exists).

### 4.9 Song properties (`song-properties`)

Page pushed from the editor. Layout: toolbar, then two columns: the form (rest) and the
checks panel (380 px, view background, 1 px border left).

- Toolbar: `[Editor (go-previous)] <-> [Upgrade to format 1.1.0… (go-up)] [Save]`. Upgrade is
  disabled with tooltip "Nothing to upgrade" when the upgrade plan is empty.
- Form (gate G-05 decides FormLayout or FormCard; the image shows FormLayout), 16 / 20 px
  padding, sections "SONG", "TIMING", "MEDIA":
  - #ARTIST, #TITLE, #LANGUAGE, #GENRE: text fields; empty shows the placeholder "empty".
  - #BPM: field + "one beat = 40.63 ms" (`smallFont`, `disabledTextColor`). Changing BPM asks
    "Keep the notes at the same time in the audio?" (Keep times / Change tempo only / Cancel).
  - #GAP: field + "ms" + flat button "Set to playhead".
  - #START / #END: two fields side by side (placeholder "-").
  - #MP3, #AUDIO, #VOCALS, #INSTRUMENTAL: text fields; a missing file shows a
    `neutralTextColor` icon `dialog-warning` with tooltip "File not found in the song folder".
  - "Other tags": `#COVER, #COVERURL, … · kept as they are` (`smallFont`, `disabledTextColor`).
  - Every edit is one undo step, applied when the field loses focus or Enter is pressed.
- Checks panel: section "CHECKS" with one InlineMessage per validation issue (Information,
  Warning or Error by severity; text from the validator; clicking an issue with a row jumps
  to that line in the editor). Section "ON SAVE": "Backup" `….txt.bak (once per session)`,
  "Encoding" `UTF-8, no BOM, LF (as read)`, "Changed rows" `63 of 612 file lines`. Section
  "UPGRADE": "Nothing to upgrade: format 1.2.0, UTF-8 without BOM, LF line endings, ends with E."
  or the list of pending upgrade changes.

### 4.10 Settings (`settings`)

`Kirigami.ScrollablePage` pushed onto the stack. Toolbar:
`[Back (go-previous)] <-> [Restore defaults (view-refresh)]`. A `Kirigami.FormLayout` with
sections (labels right aligned, field column left aligned, units as labels right of the
field, spin boxes 90 px wide, combo 230 px):

| Section | Label | Control | Default |
|---|---|---|---|
| Playback | Track when a song opens | ComboBox: "Vocals if present, else Mix", "Mix", "Instrumental if present, else Mix" | first |
| | Time added before and after a played line | SpinBox, step 0.1, 0.0-2.0, unit "s" | 0.3 |
| | Play each line you move to | Switch | off |
| Tap-along | Tap latency | SpinBox 0-300, unit "ms", button "Calibrate…" | 35 |
| | Pre-roll before the first line | SpinBox 0.0-10.0 step 0.5, unit "s" | 2.0 |
| | Speed when tap-along starts | [0.5x 0.75x 1x] | 0.5x |
| Timing health | Suspicious above | SpinBox, unit "syllables per second" | 7.0 |
| | Racing above | SpinBox, unit "syllables per second" | 10.0 |
| | Suspicious when faster than neighbours by | SpinBox, unit "times" | 2.5 |
| | Suspicious when vocals are heard less than | SpinBox, unit "% of the note time" | 40 |
| | Racing when vocals are heard less than | SpinBox, unit "% of the note time" | 20 |
| | Vocals count as heard above the noise floor by | SpinBox, unit "dB" | 12 |
| Saving | Backup | Label: "The first save of each session copies the song to "<song>.txt.bak" first." | |

Changes apply and are saved at once. Restore defaults asks with a PromptDialog
"Restore all settings to their defaults?" (buttons "Restore", "Cancel"); recent songs and
the window size are kept.

---

## 5. Dialogs

All dialogs are modal, centred on the window, close with Esc (= Cancel). Prompt dialogs are
`Kirigami.PromptDialog` with the given `dialogType`; larger ones are `Kirigami.Dialog`.
Buttons are listed in the order they must appear; the first is the default button
(Enter). Use the dialog's `customFooterActions` or `standardButtons`; if the platform reorders
standard buttons, the platform order wins.

### 5.1 Save changes? (`unsaved-changes`)

- Type Warning. Title "Save changes?". Text:
  `"<Title>" has 2 unsaved changes. If you continue without saving, they are lost.`
  (singular "1 unsaved change").
- Buttons: "Save" (`document-save`), "Discard" (`edit-delete`), "Cancel".
- Shown before: closing the window, Open…, opening a recent song, dropping a file, Reload from
  Disk. Save saves and then continues; a failed save stops (section 6) and nothing continues.

### 5.2 Song changed on disk (`changed-on-disk`)

- Type Warning. Title "Song changed on disk". Text:
  `Another program changed "<file name>" after you opened it.` and, in `disabledTextColor`:
  `Overwrite replaces those changes with yours. Reload discards your 2 unsaved changes and opens the file as it is now.`
  (without unsaved changes the second sentence is "Reload opens the file as it is now.").
- Buttons: "Overwrite" (`document-save`), "Reload" (`view-refresh`), "Cancel".
- Checked before every save and when the window becomes active again. Without unsaved
  changes, a change found on activation reloads silently and shows the passive notification
  "Reloaded: the file changed on disk."

### 5.3 Open song (file dialog)

`QtQuick.Dialogs.FileDialog` (verified available, 6.2+), title "Open song", name filters
"UltraStar songs (*.txt)" and "All files (*)", starting folder: the folder of the current
song, else of the most recent song, else the home folder.

### 5.4 Edit line (`line-text`)

- `Kirigami.Dialog`, title "Edit line 3", preferred width 720 px.
- Contents (16 px padding, 12 px spacing):
  1. Help text `smallFont`, `disabledTextColor`: "Line text. A space separates words, | splits a word into syllables, ~ continues the previous syllable on a new note."
  2. A `QQC2.TextField` with the line text (build plan 6.5 syntax), 15 px text size.
  3. Section label "SYLLABLES ON NOTES", then a `Flow` of chips (1 px border, radius 4,
     3 / 6 px padding, min width 38 px): syllable on top, start beat below in `smallFont`,
     `disabledTextColor`. Changed syllable: `highlightColor` border on highlight at 20 %;
     new note: `positiveTextColor` border on positive background.
  4. An InlineMessage (Information) explaining a count difference, for example:
     `13 syllables for 12 notes. "tho|ri|ties?" now has three syllables on two notes. The editor will split "thorities?" (25 beats) into two notes of 12 and 13 beats at the same pitch.`
     Hidden when the counts match.
  5. "When counts differ" [Split / merge longest notes | Ask me per note].
- Buttons: "Apply" (default), "Cancel". Apply is one undo step.

### 5.5 Export anchors for the Factory (`export-anchors`)

- `Kirigami.Dialog`, title "Export anchors for the Factory", preferred width 640 px.
- Contents: an Information InlineMessage "9 lines: 7 whose timing you changed, 2 pinned
  anchors. No end pin." (counts from the export content, decision D-12); the exact file text
  in a read-only `QQC2.TextArea` with `fixedWidthFont` 9 pt, comment lines in
  `disabledTextColor`; one Warning InlineMessage per check finding (build plan 6.11) with the
  action "Leave it out" where leaving the line out resolves it; a row "Save to" + read-only
  field with the path (elided in the middle) + "Change…" (`folder-open`, opens a save
  FileDialog).
- Buttons: "Export" (`document-export`, default), "Cancel". Export is disabled when the file
  would have no anchor line.
- If the target exists: PromptDialog type Warning "Replace the existing anchors.txt?" with
  "Replace" and "Cancel". After writing: passive notification "Anchors exported".

### 5.6 Upgrade to format 1.1.0 (`upgrade`)

- `Kirigami.Dialog`, title "Upgrade to format 1.1.0", preferred width 640 px.
- Contents: a `smallFont` line describing the file ("An unversioned song, Windows-1252, CRLF
  line endings."); a bullet list of the planned changes (wording as in the image, one bullet
  per change from build plan 6.12); the diff in a read-only `fixedWidthFont` area: added lines
  prefixed `+` in `positiveTextColor`, removed lines `-` in `negativeTextColor`, two context
  lines around each change, longer unchanged runs collapsed to one `disabledTextColor` line
  "… 412 unchanged lines …"; then "Nothing is written until you save. Undo reverts the
  upgrade." (`smallFont`, `disabledTextColor`).
- Buttons: "Upgrade" (`go-up`, default), "Cancel".

### 5.7 Measure tap latency (`latency-calibration`)

- `Kirigami.Dialog`, title "Measure tap latency", preferred width 470 px.
- Text: "Eight clicks play, one per second. Press Space exactly on each click." A row of 8
  circles (18 px, 2 px border in the strong grid colour, 10 px spacing): filled
  `highlightColor` when that click got a valid tap, `highlightColor` border for the click now
  playing. Under it, centred, `smallFont`, `disabledTextColor`:
  "Click 6 of 8 · measured so far: 34, 38, 36, 31, 37 ms · median 36 ms".
- Buttons: "Use 36 ms" (default, enabled after all 8 clicks have a tap; the number is the
  current median), "Start again" (`view-refresh`), "Cancel".
- A tap more than 300 ms from its click is ignored (its circle stays empty).

---

## 6. Messages and notifications (`errors-and-notifications`)

| Situation | Form | Text |
|---|---|---|
| Save failed | Error `InlineMessage` at the top of the editor column, stays until a save succeeds; actions "Try again" (`document-save`) and "Save as…" (`document-save-as`) | `Could not save: <reason>. Your changes are still here.` with reasons such as `no permission to write "<file name>"`, `the disk is full`, `the folder no longer exists` |
| Open failed | `PromptDialog` type Error, title "Cannot open song", button "OK"; the current song stays open | `"<file name>" is not an UltraStar song: it has no #BPM header.` / `"<file name>" could not be read: <reason>.` |
| Editing action refused | Passive notification (`showPassiveNotification`, 4000 ms) | `Notes cannot move before the song start. Lower #GAP in Song properties first.` / `A note needs a length of at least 1 beat.` / `Notes in different lines cannot be merged.` / `Phrase ends must stay between two notes.` |
| Saved | Passive notification, 2000 ms | `Saved` (first save of a session: `Saved. Backup written to "<file name>.bak".`) |
| Anchors exported | Passive notification, 2000 ms | `Anchors exported` |
| Reloaded after external change | Passive notification, 4000 ms | `Reloaded: the file changed on disk.` |
| Dropped file not a .txt | Passive notification, 4000 ms | `Only UltraStar song files (.txt) can be opened.` |
| Audio playback error | Warning `InlineMessage` like 4.4 | `The audio file "<name>" could not be played: <GStreamer message>.` |

Error texts say what happened and what still works or what to do; no apologies.

---

## 7. Enablement matrix

| Control | No song | Song, no audio | Read-only | Repair page | Tap recording |
|---|---|---|---|---|---|
| Open, Open Recent | ✅ | ✅ | ✅ | ✅ | ❌ |
| Save, Save As | ❌ | ✅ | ❌ | ✅ | ❌ |
| Undo, Redo | ❌ | ✅ (when available) | ❌ | ✅ | ❌ |
| Play, Play line, Loop, Track, Speed | ❌ | ❌ | ✅ | ✅ | ❌ (tap controls playback) |
| Repair timing | ❌ | ✅ (tap-along needs audio) | ❌ | - | - |
| Song properties | ❌ | ✅ | ✅ (read-only form) | ✅ | ❌ |
| Note editing (mouse, keys, inspector, line type, Edit text) | ❌ | ✅ | ❌ | ❌ (repair tools only) | ❌ |
| Export anchors | ❌ | ✅ | ✅ | ✅ | ❌ |
| Settings | ✅ | ✅ | ✅ | ✅ | ❌ |

---

## 8. Screenshot harness and acceptance procedure

### 8.1 Harness (build plan task P3-06)

Debug builds of `usdx-song-editor` accept:

```
usdx-song-editor --screenshot <out.png> --demo-state <name> [--song <file.txt>] [--size 1280x800]
```

- Runs with `QT_QPA_PLATFORM=offscreen`, `QT_QUICK_CONTROLS_STYLE=org.kde.desktop`,
  `QT_QPA_PLATFORMTHEME=kde`, opens the song (a copy in a temp folder, never the original),
  puts the app into the named demo state, waits until the window is idle (no pending
  analysis; at most 10 s), saves an image of the window's root item with `grabToImage` and
  `saveToFile`, and exits 0. (Verified on this machine: an item in an offscreen window grabs
  and saves correctly with real Breeze controls.)
- Theme: by default the user's colour scheme. For Breeze Light, run with `XDG_CONFIG_HOME`
  pointing to a temp folder that holds a `kdeglobals` copied from
  `/usr/share/color-schemes/BreezeLight.colors`; for Breeze Dark, the same with
  `BreezeDark.colors`. (Verified: this switches `Kirigami.Theme` between `#eff0f1` and
  `#202326` window backgrounds without touching the user's desktop settings.) The temp
  `XDG_CONFIG_HOME` also isolates the app settings, which is wanted.
- Pixel check (P3-06, measured correction of the original assumption): the page header
  toolbar covers the top edge of the window, so the pixel at (5, 5) is the toolbar
  background (`#dee0e2` light, `#292c30` dark) and the window background of the theme
  (`#eff0f1` light, `#202326` dark) shows at every point below it, for example (5, 60).
  The test asserts both pixels.
- Demo states reproduce the reference images: `editor` (aequitaS - Realize The Real Lies from
  the test songs, line 3 current, note 3 selected, playhead at beat 181, track Vocals, two
  committed edits), `repair-anchor` and `repair-block` (lines 15-22 compressed to 1:06.18 -
  1:12.00 as in the build plan's synthetic fault, anchors on lines 15 and 23), `tap-along`
  (12 taps recorded), `line-text`, `song-properties`, `start-screen` (three recent songs
  from the test folders), `main-menu` (Open Recent open), `audio-states-missing`,
  `audio-states-loading`, `audio-states-no-vocals`, `read-only` (a fixture with
  `#VERSION:2.0.0`), `unsaved-changes`, `changed-on-disk`, `export-anchors`, `upgrade` (fixture
  `cp1252_unversioned.txt`), `settings`, `latency-calibration` (5 taps), `save-error`,
  `refused-action`, `open-error`.
- A script `scripts/ui-shots.sh` renders every demo state in both themes into
  `target/ui-shots/<name>-<theme>.png`, with the memory guard of `scripts/dev.sh`.

### 8.2 Acceptance for every UI task

1. Render the task's screens with `scripts/ui-shots.sh <names>` in light and dark.
2. Open each image next to `Plan/mockups/<name>-<theme>.png` (an agent reads both PNG files)
   and check, in this order:
   - the same elements are present, in the same order and grouping;
   - every text matches this document exactly (`qsTr` source strings);
   - colours follow the roles in 2.3 in both themes (no hard-coded light-only colour);
   - the app-drawn parts (sheet, lanes, list rows, chips, pills, dots, flags, key cap) match
     the sizes in this document within 2 px;
   - native controls look like Breeze controls (they will differ from the mock by a few
     pixels; that is expected);
   - nothing is clipped, overlaps or wraps where this document says one line.
3. Write the result into `Plan/PROGRESS.md` for the task: the screens checked, both themes,
   and every deviation with its reason. A deviation without a reason in this document fails
   the task.
4. The user does a final visual check on the real desktop (Wayland, Plasma) for each phase;
   agents do not change the user's desktop theme or settings for this.
