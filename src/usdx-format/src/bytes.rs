//! The byte layer of the UltraStar song format.
//!
//! About me: this module decodes the raw bytes of a song file into lines with their
//! line terminators and detects the text encoding in the plan's order (UTF-8 BOM,
//! valid UTF-8, an ASCII-readable `#ENCODING` header, then a chardetng guess between
//! windows-1252 and windows-1250), and encodes a decoded file back to bytes as the
//! exact inverse. It exists so that every later layer can work on text while the
//! the editor keeps the file byte-identical unless a row was really edited. The module
//! also defines `DecodedFileMeta`, the file-level metadata the row layer carries in a
//! document so the serializer can write the BOM and the encoding back unchanged.

use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use encoding_rs::{Encoding, WINDOWS_1250, WINDOWS_1252};
use thiserror::Error;

/// The text encodings the format allows.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TextEncoding {
    Utf8,
    Windows1252,
    Windows1250,
}

/// How the encoding of a file was found.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EncodingSource {
    /// The bytes are valid UTF-8 (or carry a UTF-8 BOM).
    Valid,
    /// The `#ENCODING` header names the encoding.
    Header,
    /// chardetng guessed the encoding (shown as a warning in the UI).
    Guessed,
}

/// The line terminator of a raw line; `None` only on the last line of a file.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Terminator {
    Lf,
    CrLf,
    Cr,
    None,
}

impl Terminator {
    /// The bytes this terminator writes back.
    fn as_bytes(self) -> &'static [u8] {
        match self {
            Terminator::Lf => b"\n",
            Terminator::CrLf => b"\r\n",
            Terminator::Cr => b"\r",
            Terminator::None => b"",
        }
    }
}

/// One raw line: the decoded text without its terminator, and the terminator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawLine {
    pub text: String,
    pub terminator: Terminator,
}

/// The whole decoded file: encoding information plus the raw lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedFile {
    pub bom: bool,
    pub encoding: TextEncoding,
    pub encoding_source: EncodingSource,
    pub lines: Vec<RawLine>,
}

/// The file-level metadata of a decoded file. The row layer keeps it in
/// `Document::file` so the serializer can write the BOM and the encoding back
/// unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodedFileMeta {
    pub bom: bool,
    pub encoding: TextEncoding,
    pub encoding_source: EncodingSource,
}

impl DecodedFile {
    /// meta returns the file-level metadata of the decoded file.
    pub fn meta(&self) -> DecodedFileMeta {
        DecodedFileMeta {
            bom: self.bom,
            encoding: self.encoding,
            encoding_source: self.encoding_source,
        }
    }
}

/// Errors of the byte layer; later layers extend their own error types.
#[derive(Debug, Error)]
pub enum FormatError {
    /// The `#ENCODING` header names an encoding the format does not define.
    #[error("the #ENCODING header names the unsupported encoding {0}")]
    UnsupportedEncoding(String),
    /// The file claims to be UTF-8 but the bytes are not valid UTF-8.
    #[error("the file claims UTF-8 but the bytes are not valid UTF-8")]
    InvalidUtf8,
    /// chardetng guessed an encoding outside the ones the format allows.
    #[error("the encoding is not recognizable (guessed {0})")]
    UnrecognizedEncoding(String),
    /// Text cannot be written in the encoding of the file.
    #[error("the text cannot be written in {encoding:?}")]
    NotEncodable { encoding: TextEncoding },
}

/// The UTF-8 byte order mark.
const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// decode turns the raw bytes of a song file into a decoded file. It never fails on
/// ordinary legacy files: every byte maps to a character in the single-byte encodings,
/// so decoding only fails when the file names an encoding the format does not define,
/// claims UTF-8 without being valid UTF-8, or the guess lands outside the allowed
/// encodings.
pub fn decode(bytes: &[u8]) -> Result<DecodedFile, FormatError> {
    let (bom, rest) = match bytes.strip_prefix(&BOM) {
        Some(rest) => (true, rest),
        None => (false, bytes),
    };
    let ranges = split_ranges(rest);

    let (encoding, encoding_source) = detect_encoding(rest, ranges.first())?;

    let mut lines = Vec::with_capacity(ranges.len());
    for range in &ranges {
        lines.push(RawLine {
            text: decode_range(rest, range, encoding)?,
            terminator: range.terminator,
        });
    }

    Ok(DecodedFile {
        bom,
        encoding,
        encoding_source,
        lines,
    })
}

/// encode is the exact inverse of decode: encoding the decoded file yields the
/// original bytes, including the BOM, the encoding and every line's own terminator.
pub fn encode(file: &DecodedFile) -> Result<Vec<u8>, FormatError> {
    let mut out = Vec::new();
    if file.bom {
        out.extend_from_slice(&BOM);
    }
    for line in &file.lines {
        match file.encoding {
            TextEncoding::Utf8 => out.extend_from_slice(line.text.as_bytes()),
            TextEncoding::Windows1252 => {
                encode_into(&mut out, WINDOWS_1252, &line.text, file.encoding)?
            }
            TextEncoding::Windows1250 => {
                encode_into(&mut out, WINDOWS_1250, &line.text, file.encoding)?
            }
        }
        out.extend_from_slice(line.terminator.as_bytes());
    }
    Ok(out)
}

/// encode_into appends the text encoded with the given encoding; a character that the
/// encoding cannot represent fails loudly instead of being replaced.
fn encode_into(
    out: &mut Vec<u8>,
    encoding: &'static Encoding,
    text: &str,
    requested: TextEncoding,
) -> Result<(), FormatError> {
    let (encoded, _, had_errors) = encoding.encode(text);
    if had_errors {
        return Err(FormatError::NotEncodable {
            encoding: requested,
        });
    }
    out.extend_from_slice(&encoded);
    Ok(())
}

/// A byte range of one line inside the file, with the terminator that ends it.
struct ByteRange {
    start: usize,
    end: usize,
    terminator: Terminator,
}

/// split_ranges finds the byte ranges of the lines. LF, CRLF and CR each end a line,
/// the end of the input ends the last line with `Terminator::None`, and a file that
/// ends with a terminator produces no extra line.
fn split_ranges(bytes: &[u8]) -> Vec<ByteRange> {
    let mut ranges = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                ranges.push(ByteRange {
                    start,
                    end: i,
                    terminator: Terminator::Lf,
                });
                i += 1;
                start = i;
            }
            b'\r' => {
                let (end_terminator, step) = match bytes.get(i + 1) {
                    Some(b'\n') => (Terminator::CrLf, 2usize),
                    _ => (Terminator::Cr, 1usize),
                };
                ranges.push(ByteRange {
                    start,
                    end: i,
                    terminator: end_terminator,
                });
                i += step;
                start = i;
            }
            _ => i += 1,
        }
    }
    if start < bytes.len() {
        ranges.push(ByteRange {
            start,
            end: bytes.len(),
            terminator: Terminator::None,
        });
    }
    ranges
}

/// detect_encoding finds the encoding in the plan's order: valid UTF-8 wins, then an
/// ASCII-readable `#ENCODING` header on the first line, then the chardetng guess.
fn detect_encoding(
    bytes: &[u8],
    first_range: Option<&ByteRange>,
) -> Result<(TextEncoding, EncodingSource), FormatError> {
    if std::str::from_utf8(bytes).is_ok() {
        return Ok((TextEncoding::Utf8, EncodingSource::Valid));
    }
    if let Some(first_range) = first_range
        && let Some(named) = encoding_from_header(bytes, first_range)?
    {
        return Ok(named);
    }
    guess_encoding(bytes)
}

/// encoding_from_header reads the `#ENCODING` header from the first line, decoded as
/// windows-1252 because every byte maps to a character there and the header is
/// ASCII-readable in the files that carry it. The key and the value are
/// case-insensitive and ignore surrounding whitespace; an empty value counts as absent.
fn encoding_from_header(
    bytes: &[u8],
    first_range: &ByteRange,
) -> Result<Option<(TextEncoding, EncodingSource)>, FormatError> {
    let first_line = &WINDOWS_1252
        .decode(&bytes[first_range.start..first_range.end])
        .0;
    let first_line = first_line.trim();
    let Some((key, value)) = first_line.split_once(':') else {
        return Ok(None);
    };
    if !key
        .trim()
        .trim_start_matches('#')
        .eq_ignore_ascii_case("ENCODING")
    {
        return Ok(None);
    }
    let name = value.trim().to_ascii_uppercase();
    let encoding = match name.as_str() {
        "UTF-8" | "UTF8" => TextEncoding::Utf8,
        "CP1252" | "WINDOWS-1252" => TextEncoding::Windows1252,
        "CP1250" | "WINDOWS-1250" => TextEncoding::Windows1250,
        "" => return Ok(None),
        other => return Err(FormatError::UnsupportedEncoding(other.to_owned())),
    };
    if encoding == TextEncoding::Utf8 {
        // The bytes did not decode as UTF-8, so a UTF-8 claim is a broken file.
        return Err(FormatError::InvalidUtf8);
    }
    Ok(Some((encoding, EncodingSource::Header)))
}

/// guess_encoding asks chardetng to choose between windows-1252 and windows-1250.
/// chardetng sometimes expresses the windows-1250 conclusion through its Latin-2
/// candidate ISO-8859-2 for central-European files (decided with the user: the two
/// encodings share the letters, decoding uses the windows-1250 table either way, so
/// the round trip stays exact); every other guess fails loudly instead of silently
/// misreading the file.
fn guess_encoding(bytes: &[u8]) -> Result<(TextEncoding, EncodingSource), FormatError> {
    let mut detector = EncodingDetector::new(Iso2022JpDetection::Deny);
    detector.feed(bytes, true);
    let guessed = detector.guess(None, Utf8Detection::Deny);
    match guessed.name() {
        "windows-1252" => Ok((TextEncoding::Windows1252, EncodingSource::Guessed)),
        "windows-1250" | "iso-8859-2" => Ok((TextEncoding::Windows1250, EncodingSource::Guessed)),
        other => Err(FormatError::UnrecognizedEncoding(other.to_owned())),
    }
}

/// decode_range decodes one line's byte range with the file encoding. Line boundaries
/// are LF, CR or CRLF bytes, so a UTF-8 multi-byte sequence never crosses them.
fn decode_range(
    bytes: &[u8],
    range: &ByteRange,
    encoding: TextEncoding,
) -> Result<String, FormatError> {
    let slice = &bytes[range.start..range.end];
    match encoding {
        TextEncoding::Utf8 => std::str::from_utf8(slice)
            .map(str::to_owned)
            .map_err(|_| FormatError::InvalidUtf8),
        TextEncoding::Windows1252 => Ok(WINDOWS_1252.decode(slice).0.into_owned()),
        TextEncoding::Windows1250 => Ok(WINDOWS_1250.decode(slice).0.into_owned()),
    }
}
