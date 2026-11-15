//! About me: the in-place update rule of the list models (user decision
//! 2026-11-15, "Update in place"). Rebuilding a QML list model with a reset
//! destroys and recreates every delegate; on a song of a few hundred notes
//! that costs a visible pause on every click. When the new rows are the
//! same rows in the same order (the same identity keys), the bridge models
//! instead keep their delegates and signal only the span of rows whose data
//! changed. This module decides which of the two a new row list needs; it
//! holds no Qt types so the rule is unit-testable.

/// What a list model does with a new row list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowUpdate {
    /// The rows are equal: nothing to signal.
    Unchanged,
    /// The same rows in the same order; the rows `first..=last` hold
    /// changed data (one dataChanged over that span).
    Changed { first: usize, last: usize },
    /// The row count or the row identities differ: a full model reset.
    Reset,
}

/// row_update compares the old and new rows of a model by their identity
/// `key` and their data.
pub fn row_update<T: PartialEq, K: PartialEq>(
    old: &[T],
    new: &[T],
    key: impl Fn(&T) -> K,
) -> RowUpdate {
    if old.len() != new.len() {
        return RowUpdate::Reset;
    }
    let mut span: Option<(usize, usize)> = None;
    for (index, (before, after)) in old.iter().zip(new).enumerate() {
        if key(before) != key(after) {
            return RowUpdate::Reset;
        }
        if before != after {
            span = Some(match span {
                Some((first, _)) => (first, index),
                None => (index, index),
            });
        }
    }
    match span {
        Some((first, last)) => RowUpdate::Changed { first, last },
        None => RowUpdate::Unchanged,
    }
}
