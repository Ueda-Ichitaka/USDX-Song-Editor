//! About me: the analysis cache of the usdx-audio library (plan section 7.2,
//! task P4-03). It stores the peak and activity analysis of an audio file
//! under the user's cache directory, keyed by a hash of the canonical file
//! path, size and mtime, in a simple versioned binary layout. A corrupt or
//! old-version cache file is ignored and rewritten, so an analysis only ever
//! has to run once per file version.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::analysis::{Activity, Peaks};

/// The magic bytes of a cache file.
const MAGIC: &[u8; 8] = b"USDXPEAK";
/// The cache layout version; a file with another version is ignored.
const VERSION: u32 = 1;
/// The cache directory of the app below the user's cache directory.
const APP_DIR: &str = "io.github.ueda_ichitaka.UsdxSongEditor";
/// The FNV-1a 64 bit offset basis and prime.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a 64 bit over `bytes`, continuing from `hash`.
fn fnv1a(hash: u64, bytes: &[u8]) -> u64 {
    let mut hash = hash;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// The cache key of an audio file: a hash of the canonical path, size and
/// mtime; None when the file has no canonical path or no readable mtime.
fn cache_key(path: &Path) -> Option<u64> {
    let canonical = fs::canonicalize(path).ok()?;
    let metadata = fs::metadata(&canonical).ok()?;
    let mut hash = fnv1a(FNV_OFFSET, canonical.to_string_lossy().as_bytes());
    hash = fnv1a(hash, &metadata.len().to_le_bytes());
    let mtime = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos() as u64;
    hash = fnv1a(hash, &mtime.to_le_bytes());
    Some(hash)
}

/// The cache file of an audio file; None when there is no cache directory.
pub(crate) fn cache_file(path: &Path) -> Option<PathBuf> {
    let key = cache_key(path)?;
    Some(
        dirs::cache_dir()?
            .join(APP_DIR)
            .join("peaks")
            .join(format!("{key:016x}.bin")),
    )
}

fn read_u32(cursor: &mut impl Read) -> Option<u32> {
    let mut bytes = [0; 4];
    cursor.read_exact(&mut bytes).ok()?;
    Some(u32::from_le_bytes(bytes))
}

fn read_u64(cursor: &mut impl Read) -> Option<u64> {
    let mut bytes = [0; 8];
    cursor.read_exact(&mut bytes).ok()?;
    Some(u64::from_le_bytes(bytes))
}

fn read_f32(cursor: &mut impl Read) -> Option<f32> {
    let mut bytes = [0; 4];
    cursor.read_exact(&mut bytes).ok()?;
    Some(f32::from_le_bytes(bytes))
}

fn read_f64(cursor: &mut impl Read) -> Option<f64> {
    let mut bytes = [0; 8];
    cursor.read_exact(&mut bytes).ok()?;
    Some(f64::from_le_bytes(bytes))
}

/// Reads a f32 array written as consecutive little endian values.
fn read_f32_array(cursor: &mut impl Read, count: u64) -> Option<Vec<f32>> {
    // A corrupt or hostile count must not make the loader allocate gigabytes.
    if count > 200_000_000 {
        return None;
    }
    let mut values = vec![0.0_f32; count as usize];
    for value in &mut values {
        *value = read_f32(cursor)?;
    }
    Some(values)
}

/// Loads the cached analysis of an audio file, when a valid current-version
/// cache file exists for it and its bucket length matches.
pub(crate) fn load(path: &Path, bucket_ms: f64) -> Option<(Peaks, Activity)> {
    let bytes = fs::read(cache_file(path)?).ok()?;
    let mut cursor = &bytes[..];
    let mut magic = [0; 8];
    cursor.read_exact(&mut magic).ok()?;
    if magic != *MAGIC {
        return None;
    }
    if read_u32(&mut cursor)? != VERSION {
        return None;
    }
    let stored_bucket = read_f64(&mut cursor)?;
    if (stored_bucket - bucket_ms).abs() > 1e-9 {
        return None;
    }
    let count = read_u64(&mut cursor)?;
    let min = read_f32_array(&mut cursor, count)?;
    let max = read_f32_array(&mut cursor, count)?;
    let rms = read_f32_array(&mut cursor, count)?;
    let activity_bucket = read_f64(&mut cursor)?;
    let windows = read_u64(&mut cursor)?;
    if windows > count + 1 {
        return None;
    }
    let mut active = Vec::with_capacity(windows as usize);
    for _ in 0..windows {
        let mut byte = [0; 1];
        cursor.read_exact(&mut byte).ok()?;
        active.push(byte[0] != 0);
    }
    let floor_db = read_f32(&mut cursor)?;
    let peaks = Peaks {
        bucket_ms: stored_bucket,
        min,
        max,
        rms,
    };
    let activity = Activity {
        bucket_ms: activity_bucket,
        active,
        floor_db,
    };
    Some((peaks, activity))
}

/// Stores the analysis of an audio file in the cache; failures are ignored,
/// the cache is best effort. The file is written to a temporary name and
/// renamed, so a crash never leaves a half-written cache behind.
pub(crate) fn store(path: &Path, bucket_ms: f64, peaks: &Peaks, activity: &Activity) {
    let Some(file) = cache_file(path) else {
        return;
    };
    let Some(dir) = file.parent() else {
        return;
    };
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&bucket_ms.to_le_bytes());
    out.extend_from_slice(&(peaks.min.len() as u64).to_le_bytes());
    for value in &peaks.min {
        out.extend_from_slice(&value.to_le_bytes());
    }
    for value in &peaks.max {
        out.extend_from_slice(&value.to_le_bytes());
    }
    for value in &peaks.rms {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.extend_from_slice(&activity.bucket_ms.to_le_bytes());
    out.extend_from_slice(&(activity.active.len() as u64).to_le_bytes());
    for flag in &activity.active {
        out.push(u8::from(*flag));
    }
    out.extend_from_slice(&activity.floor_db.to_le_bytes());
    let temporary = file.with_extension("bin.tmp");
    if fs::write(&temporary, &out).is_ok() {
        let _ = fs::rename(&temporary, &file);
    } else {
        let _ = fs::remove_file(&temporary);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyze;
    use crate::testmedia;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    fn no_cancel() -> AtomicBool {
        AtomicBool::new(false)
    }

    /// Plan test 4: a cache hit returns identical data - the second analysis
    /// of the same file comes from the cache and equals the first.
    #[test]
    fn cache_hit_returns_identical_data() {
        let dir = tempfile::tempdir().expect("the temporary directory is created");
        let path = dir.path().join("tone.opus");
        testmedia::tone_opus(&path).expect("the test tone is generated");
        let first = analyze(&path, 10.0, &no_cancel(), &|_| {}).expect("the analysis succeeds");
        let cache = cache_file(&path).expect("the cache file has a path");
        assert!(cache.is_file(), "the analysis stored a cache file");
        let second = analyze(&path, 10.0, &no_cancel(), &|_| {}).expect("the analysis succeeds");
        assert_eq!(first, second, "the cached analysis equals the fresh one");
    }

    /// Plan test 5: a changed mtime invalidates the cache - the analysis
    /// runs again and rewrites the cache file.
    #[test]
    fn changed_mtime_invalidates_the_cache() {
        let dir = tempfile::tempdir().expect("the temporary directory is created");
        let path = dir.path().join("tone.opus");
        testmedia::tone_opus(&path).expect("the test tone is generated");
        analyze(&path, 10.0, &no_cancel(), &|_| {}).expect("the analysis succeeds");
        let cache = cache_file(&path).expect("the cache file has a path");
        let before = std::fs::metadata(&cache)
            .expect("the cache file exists")
            .modified()
            .expect("the cache file has an mtime");
        // A change to the audio file five seconds in the future changes the
        // hash key, no matter the filesystem timestamp granularity.
        let file = std::fs::File::options()
            .write(true)
            .open(&path)
            .expect("the audio file opens for writing");
        file.set_modified(std::time::SystemTime::now() + Duration::from_secs(5))
            .expect("the audio file mtime changes");
        drop(file);
        // The mtime change makes the cache key point at a new file, so the
        // cache file path is looked up again after the analysis.
        analyze(&path, 10.0, &no_cancel(), &|_| {}).expect("the analysis succeeds");
        let cache = cache_file(&path).expect("the cache file has a path");
        let after = std::fs::metadata(&cache)
            .expect("the cache file exists")
            .modified()
            .expect("the cache file has an mtime");
        assert!(
            after > before,
            "the analysis rewrites the cache file when the mtime changed"
        );
    }

    /// Plan test 6: a corrupt cache file is ignored and rewritten - the
    /// analysis after the corruption returns the same data as before and
    /// heals the cache file.
    #[test]
    fn corrupt_cache_is_ignored() {
        let dir = tempfile::tempdir().expect("the temporary directory is created");
        let path = dir.path().join("tone.opus");
        testmedia::tone_opus(&path).expect("the test tone is generated");
        let first = analyze(&path, 10.0, &no_cancel(), &|_| {}).expect("the analysis succeeds");
        let cache = cache_file(&path).expect("the cache file has a path");
        std::fs::write(&cache, b"garbage").expect("the cache file is overwritten");
        let second = analyze(&path, 10.0, &no_cancel(), &|_| {}).expect("the analysis succeeds");
        assert_eq!(first, second, "the analysis ignores the corrupt cache");
        let healed = std::fs::read(&cache).expect("the cache file exists");
        assert_ne!(
            healed, b"garbage",
            "the analysis rewrites the corrupt cache file"
        );
    }
}
