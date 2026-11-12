//! The header view of the UltraStar song format.
//!
//! About me: this module indexes the header rows of a document for the
//! case-insensitive lookup that the song view, the timing and the properties
//! panel use, reports duplicate headers for the validation layer, and parses
//! the `#VERSION` header and the format's decimal values (which accept the v1
//! decimal comma). It exists so that header consumers work on values while the
//! document keeps every header row verbatim.

use std::collections::HashMap;

use crate::rows::{Document, RowId, RowKind};

/// A later header row for a key that the document already carries. The first
/// occurrence wins the lookup; the validation layer reports the duplicates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuplicateHeader {
    pub key: String,
    pub row: RowId,
}

/// The header view of a document: one value per key, keyed by the upper-cased
/// key. An empty value is equivalent to the header being absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Headers {
    entries: HashMap<String, String>,
    duplicates: Vec<DuplicateHeader>,
}

impl Headers {
    /// from_document indexes the header rows of a document. The first header
    /// row with a non-empty value wins; every later non-empty value for the
    /// same key is a duplicate.
    pub fn from_document(doc: &Document) -> Headers {
        let mut entries = HashMap::new();
        let mut duplicates = Vec::new();
        for row in &doc.rows {
            let RowKind::Header { key, value } = &row.kind else {
                continue;
            };
            if value.is_empty() {
                continue;
            }
            if entries.contains_key(key) {
                duplicates.push(DuplicateHeader {
                    key: key.clone(),
                    row: row.id,
                });
            } else {
                entries.insert(key.clone(), value.clone());
            }
        }
        Headers {
            entries,
            duplicates,
        }
    }

    /// get returns the value of the first occurrence of the key. The key is
    /// case-insensitive; an empty value counts as absent.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(&key.to_uppercase()).map(String::as_str)
    }

    /// duplicates lists the later header rows for keys the document already
    /// carries, each with its row id.
    pub fn duplicates(&self) -> &[DuplicateHeader] {
        &self.duplicates
    }
}

/// The file format version of a document, parsed from the `#VERSION` header.
/// The absence of the header (or an empty value) means the unversioned format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Version {
    Unversioned,
    V(u32, u32, u32),
    /// A syntactically invalid value, kept for the message of the validation
    /// layer.
    Invalid(String),
}

impl Version {
    /// parse reads a `#VERSION` value: three number parts separated by periods,
    /// as the v1 format defines it. The caller passes the trimmed value.
    pub fn parse(value: &str) -> Version {
        if value.is_empty() {
            return Version::Unversioned;
        }
        let mut parts = value.split('.');
        let mut numbers = [0u32; 3];
        for number in &mut numbers {
            let Some(part) = parts.next() else {
                return Version::Invalid(value.to_owned());
            };
            match part.parse() {
                Ok(parsed) => *number = parsed,
                Err(_) => return Version::Invalid(value.to_owned()),
            }
        }
        if parts.next().is_some() {
            return Version::Invalid(value.to_owned());
        }
        Version::V(numbers[0], numbers[1], numbers[2])
    }
}

/// parse_decimal parses a format decimal value: one or more digits, optionally
/// followed by a period or comma and one or more fraction digits, as the v1
/// format defines it for `#BPM`, `#GAP`, `#START` and `#END`. The syntax has no
/// sign; the validation layer decides what a negative value means.
pub fn parse_decimal(value: &str) -> Option<f64> {
    let Some(separator) = value.find(['.', ',']) else {
        if !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()) {
            return value.parse().ok();
        }
        return None;
    };
    let integer_part = &value[..separator];
    // The period and the comma are one byte wide, so the fraction starts one
    // byte after the separator.
    let fraction_part = &value[separator + 1..];
    let digits = |part: &str| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit());
    if !digits(integer_part) || !digits(fraction_part) {
        return None;
    }
    format!("{integer_part}.{fraction_part}").parse().ok()
}
