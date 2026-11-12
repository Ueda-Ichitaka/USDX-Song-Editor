//! The file fingerprint of the save service.
//!
//! About me: this module fingerprints a loaded song file by size,
//! modification time and an FNV-1a content hash. It exists so the editor can
//! detect that a file changed on disk between load and save without keeping
//! the whole file content around.

use std::time::SystemTime;

/// The fingerprint of a file's content at one point in time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileFingerprint {
    pub size: u64,
    pub modified: Option<SystemTime>,
    /// The FNV-1a hash of the file bytes; a cheap change detector, not a
    /// cryptographic checksum.
    pub hash: u64,
}

impl FileFingerprint {
    /// new fingerprints a file from its metadata and bytes.
    pub fn new(size: u64, modified: Option<SystemTime>, bytes: &[u8]) -> FileFingerprint {
        FileFingerprint {
            size,
            modified,
            hash: fnv1a(bytes),
        }
    }
}

/// fnv1a hashes the bytes with the 64-bit FNV-1a parameters.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
