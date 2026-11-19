// About me: the one list of every key binding and mouse gesture of the
// editor window (UI_SPEC 4.11, round R-08). The keyboard shortcuts page
// renders it; entries with `shortcut: true` are window shortcuts (a QML
// Shortcut or a Kirigami.Action shortcut), and the self-test compares their
// sequences (`match`, else `keys`) with the window's shortcuts in both
// directions, so the list cannot drift from the code. Keys handled inside a
// text field, the tap-along page or a dialog and the mouse gestures are
// listed for the reader only.
.pragma library

// groups lists the page's sections in order. An entry has `keys` (the
// alternatives, each a key sequence in Qt's portable text, or a gesture text
// for `mouse` entries), `action`, and optionally `shortcut`, `match` and
// `mouse`.
const groups = [
    {
        title: "Playback",
        entries: [
            { keys: ["Space"], action: "Play / pause", shortcut: true },
            { keys: ["P"], action: "Play the current line with 0.3 s before and after", shortcut: true },
            { keys: ["L"], action: "Loop the current line on / off", shortcut: true },
            { keys: ["V"], action: "Next track (Mix, Vocals, Instrumental; missing ones are skipped)", shortcut: true },
            { keys: ["[", "]"], action: "Slower / faster (0.5x, 0.75x, 1x)", shortcut: true },
            { keys: ["Home", "End"], action: "Go to the song start / the song end", shortcut: true },
            { keys: ["Shift+A"], action: "Autoplay every line you step to on / off", shortcut: true }
        ]
    },
    {
        title: "Navigation",
        entries: [
            { keys: ["Up", "Down"], action: "Previous / next line", shortcut: true },
            { keys: ["Left", "Right"], action: "Previous / next note; left of the first note selects the whole line", shortcut: true },
            { keys: ["Esc"], action: "End Add note, else stop the playback, else clear the selection; on a page like this one: go back", shortcut: true }
        ]
    },
    {
        title: "Notes",
        entries: [
            { keys: ["N"], action: "Normal note", shortcut: true },
            { keys: ["G"], action: "Golden note", shortcut: true },
            { keys: ["F"], action: "Freestyle on / off", shortcut: true },
            { keys: ["R"], action: "Rap note", shortcut: true },
            { keys: ["Shift+R"], action: "Rap golden note", shortcut: true },
            { keys: ["Ctrl+Left", "Ctrl+Right"], action: "Move the selection one beat earlier / later", shortcut: true },
            { keys: ["Ctrl+Shift+Left", "Ctrl+Shift+Right"], action: "Move the selection and everything after it one beat", shortcut: true },
            { keys: ["Ctrl+Up", "Ctrl+Down"], action: "Pitch one semitone up / down", shortcut: true },
            { keys: ["Ctrl+Shift+Up", "Ctrl+Shift+Down"], action: "Pitch one octave up / down", shortcut: true },
            { keys: ["Alt+Left", "Alt+Right"], action: "Length one beat shorter / longer", shortcut: true },
            { keys: ["S"], action: "Split the selected note at the playhead", shortcut: true },
            { keys: ["M"], action: "Merge the selected note with the next", shortcut: true },
            { keys: ["Insert"], action: "Insert a note after the selected one (or after the line's last note)", shortcut: true },
            { keys: ["Delete"], action: "Delete the selected notes", shortcut: true },
            { keys: ["B"], action: "Line break (phrase end) before the selected note on / off", shortcut: true },
            { keys: ["Ctrl+Z"], action: "Undo", shortcut: true },
            { keys: ["Ctrl+Shift+Z"], action: "Redo", shortcut: true }
        ]
    },
    {
        title: "Text",
        entries: [
            { keys: ["Enter"], match: ["Return", "Enter"], action: "Edit the selected note's syllable", shortcut: true },
            { keys: ["Ctrl+E"], action: "Edit the current line's text in the lyrics lane", shortcut: true },
            { keys: ["Enter", "Esc"], action: "In a text field: apply / cancel" },
            { keys: ["Tab", "Shift+Tab"], action: "In the syllable field: apply and edit the next / previous note" }
        ]
    },
    {
        title: "Timing repair and tap-along",
        entries: [
            { keys: ["Ctrl+R"], action: "Repair timing", shortcut: true },
            { keys: ["T"], action: "Tap-along for the racing section, else the current line", shortcut: true },
            { keys: ["Space"], action: "On the tap-along page: tap; hold it for the note length" },
            { keys: ["Backspace"], action: "On the tap-along page: take the last tap back" },
            { keys: ["Space"], action: "In Measure tap latency: tap on each click" }
        ]
    },
    {
        title: "Files and window",
        entries: [
            { keys: ["Ctrl+O"], action: "Open a song", shortcut: true },
            { keys: ["Ctrl+S"], action: "Save", shortcut: true },
            { keys: ["Ctrl+Shift+S"], action: "Save as", shortcut: true },
            { keys: ["Ctrl+,"], action: "Settings", shortcut: true },
            { keys: ["F1", "Ctrl+?"], action: "This page", shortcut: true },
            { keys: ["Ctrl+Q"], action: "Quit", shortcut: true }
        ]
    },
    {
        title: "Mouse",
        entries: [
            { mouse: true, keys: ["Click a note"], action: "Select it; a note of another line makes that line current" },
            { mouse: true, keys: ["Ctrl+click a note"], action: "Add it to the selection" },
            { mouse: true, keys: ["Shift+click a note"], action: "Select every note from the last clicked one to this one" },
            { mouse: true, keys: ["Drag on empty space"], action: "Select every note in the rectangle" },
            { mouse: true, keys: ["Shift+drag on empty space"], action: "Draw a new note on that row, from where you press to where you let go" },
            { mouse: true, keys: ["Shift+click on empty space"], action: "Add a 4-beat note" },
            { mouse: true, keys: ["Add note, then click or drag"], action: "Place one new note anywhere on the pitch rows" },
            { mouse: true, keys: ["Drag a note"], action: "Move it; hold Shift to keep the first direction" },
            { mouse: true, keys: ["Drag the end of a selected note"], action: "Resize it from that end; hold Alt to turn the snapping off" },
            { mouse: true, keys: ["Drag a dashed line"], action: "Move the phrase end between two lines" },
            { mouse: true, keys: ["Double-click a note or syllable"], action: "Edit its syllable" },
            { mouse: true, keys: ["Click or drag the ruler or the waveform"], action: "Move the playhead" },
            { mouse: true, keys: ["Wheel"], action: "Scroll in time" },
            { mouse: true, keys: ["Ctrl+wheel"], action: "Zoom around the pointer" },
            { mouse: true, keys: ["Shift+wheel"], action: "Scroll the pitch rows" },
            { mouse: true, keys: ["Middle drag"], action: "Pan" }
        ]
    }
];

// shortcutSequences lists the key sequences of every window shortcut entry,
// for the self-test's comparison with the code.
function shortcutSequences() {
    const sequences = [];
    for (const group of groups) {
        for (const entry of group.entries) {
            if (entry.shortcut) {
                for (const sequence of (entry.match || entry.keys)) {
                    sequences.push(sequence);
                }
            }
        }
    }
    return sequences;
}

// entryCount counts the entries of every group.
function entryCount() {
    return groups.reduce((sum, group) => sum + group.entries.length, 0);
}
