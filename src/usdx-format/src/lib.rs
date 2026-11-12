//! The usdx-format library.
//!
//! About me: this crate holds the lossless UltraStar Deluxe song file model: the byte
//! layer (encoding detection and line terminators), row classification, the header and
//! song views, beat-to-millisecond timing, the byte-exact serializer and the validation
//! rules. It exists so that the editor and the edit operations in usdx-edit work on
//! files that save back byte-for-byte unchanged unless a row was really edited.
//!
//! This skeleton is filled in by tasks P1-01 to P1-09 of Plan/IMPLEMENTATION_PLAN.md.

pub mod bytes;
pub mod headers;
pub mod rows;
pub mod song;
pub mod timing;
