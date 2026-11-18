// About me: the placement of the line text editor's preview strip (UI_SPEC
// 5.4, user decision 2026-11-18). The strip shows one chip per syllable under
// the note it lands on, so the chips follow the sheet's beat-to-x mapping;
// the helper is a plain function so tests/qml/tst_syllables.qml can pin it.
.pragma library

// stripLayout places one chip per beat (in ascending order): x is the sheet x
// of the beat (originX is the gutter's right edge, where startBeat lies),
// never left of originX; maxWidth ends 2 px before the next chip's x (never
// negative), the last chip is unbounded.
function stripLayout(beats, startBeat, pxPerBeat, originX) {
    const xs = beats.map((beat) => Math.max(originX, originX + (beat - startBeat) * pxPerBeat));
    return xs.map((x, index) => ({
        x: x,
        maxWidth: index + 1 < xs.length ? Math.max(0, xs[index + 1] - x - 2) : Infinity
    }));
}
