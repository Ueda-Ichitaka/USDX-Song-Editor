//! About me: tests for the header view (task P1-03, plan sections 5.1 and 5.4).
//! They assert the case-insensitive lookup, the first-occurrence-wins rule with
//! the duplicate report, the absent-on-empty-value rule, the `#VERSION` parsing
//! (unversioned, 1.0.0, 1.1.0, 1.2.0, 2.0.0, garbage) and the decimal parsing
//! that accepts the v1 decimal comma.

use usdx_format::bytes::decode;
use usdx_format::headers::{DuplicateHeader, Headers, Version, parse_decimal};
use usdx_format::rows::{RowId, classify};

/// headers builds the header view of a decoded text.
fn headers(text: &str) -> Headers {
    Headers::from_document(&classify(
        decode(text.as_bytes()).expect("the text decodes"),
    ))
}

/// doc classifies a decoded text.
fn doc(text: &str) -> usdx_format::rows::Document {
    classify(decode(text.as_bytes()).expect("the text decodes"))
}

// ---- Lookup ----

#[test]
fn the_lookup_is_case_insensitive() {
    let h = headers("#TITLE:My Song\nE\n");
    assert_eq!(h.get("TITLE"), Some("My Song"));
    assert_eq!(h.get("title"), Some("My Song"));
    assert_eq!(h.get("Title"), Some("My Song"));
}

#[test]
fn an_unknown_header_is_absent() {
    let h = headers("#TITLE:x\nE\n");
    assert_eq!(h.get("ARTIST"), None);
}

#[test]
fn an_unknown_header_line_is_still_available_by_its_key() {
    // Unknown headers MUST be ignored by readers, but the view keeps them so
    // the serializer and the properties panel can work with them.
    let h = headers("#XUNKNOWN:1\nE\n");
    assert_eq!(h.get("xunknown"), Some("1"));
}

#[test]
fn the_value_is_trimmed_by_the_classification() {
    let h = headers("#TITLE :  My Song \nE\n");
    assert_eq!(h.get("TITLE"), Some("My Song"));
}

// ---- First occurrence wins, duplicates reported ----

#[test]
fn the_first_occurrence_wins_and_later_ones_are_reported() {
    let d = doc("#TITLE:A\n#TITLE:B\nE\n");
    let h = Headers::from_document(&d);
    assert_eq!(h.get("TITLE"), Some("A"));
    assert_eq!(
        h.duplicates(),
        [DuplicateHeader {
            key: "TITLE".to_owned(),
            row: RowId(1)
        }]
    );
}

#[test]
fn duplicates_are_reported_per_occurrence() {
    let d = doc("#TITLE:A\n#TITLE:B\n#TITLE:C\nE\n");
    let h = Headers::from_document(&d);
    assert_eq!(h.duplicates().len(), 2);
    assert_eq!(h.duplicates()[1].row, RowId(2));
}

#[test]
fn duplicates_are_detected_across_letter_case() {
    let d = doc("#Title:A\n#TITLE:B\nE\n");
    let h = Headers::from_document(&d);
    assert_eq!(h.get("TITLE"), Some("A"));
    assert_eq!(h.duplicates().len(), 1);
    assert_eq!(h.duplicates()[0].key, "TITLE");
}

#[test]
fn different_keys_are_not_duplicates() {
    let h = headers("#TITLE:A\n#ARTIST:B\nE\n");
    assert!(h.duplicates().is_empty());
}

// ---- Empty value equals absent ----

#[test]
fn an_empty_value_is_absent() {
    let h = headers("#TITLE:\nE\n");
    assert_eq!(h.get("TITLE"), None);
    assert!(h.duplicates().is_empty());
}

#[test]
fn an_empty_value_does_not_shadow_a_later_value() {
    let h = headers("#TITLE:\n#TITLE:B\nE\n");
    assert_eq!(h.get("TITLE"), Some("B"));
    assert!(h.duplicates().is_empty());
}

#[test]
fn an_empty_value_after_a_value_is_not_a_duplicate() {
    let h = headers("#TITLE:A\n#TITLE:\nE\n");
    assert_eq!(h.get("TITLE"), Some("A"));
    assert!(h.duplicates().is_empty());
}

// ---- Version parsing ----

#[test]
fn a_missing_version_header_is_unversioned() {
    let h = headers("#TITLE:x\nE\n");
    assert_eq!(h.get("VERSION"), None);
}

#[test]
fn the_published_versions_parse() {
    assert_eq!(Version::parse("1.0.0"), Version::V(1, 0, 0));
    assert_eq!(Version::parse("1.1.0"), Version::V(1, 1, 0));
    assert_eq!(Version::parse("1.2.0"), Version::V(1, 2, 0));
    assert_eq!(Version::parse("2.0.0"), Version::V(2, 0, 0));
}

#[test]
fn an_empty_version_value_is_unversioned() {
    // An empty value is equivalent to the header being absent.
    assert_eq!(Version::parse(""), Version::Unversioned);
}

#[test]
fn leading_zeros_in_a_version_are_digits() {
    assert_eq!(Version::parse("01.02.003"), Version::V(1, 2, 3));
}

#[test]
fn a_version_needs_three_number_parts() {
    assert!(matches!(Version::parse("1.0"), Version::Invalid(_)));
    assert!(matches!(Version::parse("1.0.0.0"), Version::Invalid(_)));
}

#[test]
fn garbage_version_values_are_invalid() {
    assert!(matches!(Version::parse("garbage"), Version::Invalid(_)));
    assert!(matches!(Version::parse("1.0.x"), Version::Invalid(_)));
    assert!(matches!(Version::parse("1 .0.0"), Version::Invalid(_)));
    assert!(matches!(Version::parse("1.0.0-beta"), Version::Invalid(_)));
}

#[test]
fn a_version_part_that_overflows_u32_is_invalid() {
    assert!(matches!(
        Version::parse("4294967296.0.0"),
        Version::Invalid(_)
    ));
}

// ---- Decimal values with the v1 decimal comma ----

#[test]
fn a_decimal_comma_parses() {
    assert_eq!(parse_decimal("369,14"), Some(369.14));
}

#[test]
fn a_decimal_period_parses() {
    assert_eq!(parse_decimal("369.14"), Some(369.14));
}

#[test]
fn an_integer_parses() {
    assert_eq!(parse_decimal("14937"), Some(14_937.0));
}

#[test]
fn a_bpm_value_with_comma_comes_through_the_view() {
    let h = headers("#BPM:369,14\nE\n");
    assert_eq!(h.get("BPM").and_then(parse_decimal), Some(369.14));
}

#[test]
fn broken_decimals_do_not_parse() {
    assert_eq!(parse_decimal("369."), None);
    assert_eq!(parse_decimal(",5"), None);
    assert_eq!(parse_decimal(".5"), None);
    assert_eq!(parse_decimal("1.2.3"), None);
    assert_eq!(parse_decimal("1,2,3"), None);
    assert_eq!(parse_decimal(""), None);
    assert_eq!(parse_decimal(" 369"), None);
    assert_eq!(parse_decimal("+5"), None);
    // The time header syntax has no minus sign; the validation layer decides
    // what a negative value means.
    assert_eq!(parse_decimal("-5"), None);
}
