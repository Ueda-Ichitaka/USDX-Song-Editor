//! The block move and scale of the timing repair (plan section 6.8, task
//! P7-03).
//!
//! About me: a block of a voice's lines moves or scales onto a new time
//! span as one edit: the block's first note start pins the new start and
//! its last note end pins the new end, so the block's internal times map
//! linearly (a pure move is the k = 1 case). The lines before the block
//! keep their place; the lines after follow the block (MoveAlong shifts
//! them by the end offset) or keep theirs (Stay, where the collision pass
//! resolves the overlap at the boundary over the retimed lines plus one
//! line before and after them, user decision 2026-11-15). The transform is
//! an anchor stretch (6.7) with a start-pinned and an end-pinned anchor,
//! so the single-source mapping, rounding, marker placement and write-back
//! all live in retime.rs.

use std::ops::Range;

use crate::patch::{EditError, Patch};
use crate::retime::{Anchor, Lengths, Outside, Pin, StretchOptions, anchor_stretch};
use usdx_format::rows::Document;
use usdx_format::song::Song;

/// Follow says what happens to the lines after the block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Follow {
    /// The lines keep their place; the collision pass resolves the overlap
    /// at the block's new end.
    Stay,
    /// The lines shift by the block's end offset.
    MoveAlong,
}

/// block_transform plans the move or scale of `voice`'s lines in `range`
/// onto the span from `new_start_ms` to `new_end_ms` and returns the
/// undoable patch.
pub fn block_transform(
    doc: &Document,
    song: &Song,
    voice: u8,
    lines: Range<usize>,
    new_start_ms: f64,
    new_end_ms: f64,
    follow: Follow,
) -> Result<Patch, EditError> {
    if !new_start_ms.is_finite() || !new_end_ms.is_finite() {
        return Err(EditError::InvalidHeaderValue);
    }
    let voice_lines = song.voices.get(usize::from(voice)).map(|v| v.lines.len());
    let in_range = voice_lines.is_some_and(|count| lines.start < lines.end && lines.end <= count);
    if !in_range {
        return Err(EditError::StalePatch(
            "the block names lines the voice does not have",
        ));
    }
    anchor_stretch(
        doc,
        song,
        voice,
        &[
            Anchor {
                line: lines.start,
                target_ms: new_start_ms,
                pin: Pin::Start,
            },
            Anchor {
                line: lines.end - 1,
                target_ms: new_end_ms,
                pin: Pin::End,
            },
        ],
        &StretchOptions {
            before: Outside::Keep,
            after: match follow {
                Follow::Stay => Outside::Keep,
                Follow::MoveAlong => Outside::Shift,
            },
            lengths: Lengths::Scale,
            end_pin_ms: None,
        },
    )
}
