//! About me: tests for the in-place list model update (user decision
//! 2026-11-15, "Update in place"). A model keeps its delegates when the new
//! rows are the same rows in the same order (same identity keys): it then
//! reports only the span of rows whose data changed, or nothing at all. A
//! different row count or a different identity at any position needs a
//! full reset.

use usdx_song_editor::controller::model_update::{RowUpdate, row_update};

/// A row with an identity key and some data.
#[derive(Debug, Clone, PartialEq)]
struct Row {
    id: u32,
    value: i32,
}

fn rows(values: &[(u32, i32)]) -> Vec<Row> {
    values
        .iter()
        .map(|&(id, value)| Row { id, value })
        .collect()
}

fn key(row: &Row) -> u32 {
    row.id
}

#[test]
fn identical_rows_need_nothing() {
    let old = rows(&[(1, 10), (2, 20)]);
    assert_eq!(row_update(&old, &old.clone(), key), RowUpdate::Unchanged);
}

#[test]
fn one_changed_row_reports_its_index() {
    let old = rows(&[(1, 10), (2, 20), (3, 30)]);
    let new = rows(&[(1, 10), (2, 21), (3, 30)]);
    assert_eq!(
        row_update(&old, &new, key),
        RowUpdate::Changed { first: 1, last: 1 }
    );
}

#[test]
fn several_changed_rows_report_their_span() {
    let old = rows(&[(1, 10), (2, 20), (3, 30), (4, 40)]);
    let new = rows(&[(1, 11), (2, 20), (3, 31), (4, 40)]);
    assert_eq!(
        row_update(&old, &new, key),
        RowUpdate::Changed { first: 0, last: 2 }
    );
}

#[test]
fn a_different_row_count_resets() {
    let old = rows(&[(1, 10), (2, 20)]);
    let new = rows(&[(1, 10), (2, 20), (3, 30)]);
    assert_eq!(row_update(&old, &new, key), RowUpdate::Reset);
    assert_eq!(row_update(&new, &old, key), RowUpdate::Reset);
}

#[test]
fn reordered_rows_reset() {
    let old = rows(&[(1, 10), (2, 20)]);
    let new = rows(&[(2, 20), (1, 10)]);
    assert_eq!(row_update(&old, &new, key), RowUpdate::Reset);
}

#[test]
fn a_replaced_identity_resets() {
    let old = rows(&[(1, 10), (2, 20)]);
    let new = rows(&[(1, 10), (5, 20)]);
    assert_eq!(row_update(&old, &new, key), RowUpdate::Reset);
}

#[test]
fn empty_lists_need_nothing() {
    let empty: Vec<Row> = Vec::new();
    assert_eq!(row_update(&empty, &empty, key), RowUpdate::Unchanged);
}
