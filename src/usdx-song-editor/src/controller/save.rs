//! The save service of the editor.
//!
//! About me: this module writes a loaded song back to disk the way plan
//! section 8.7 defines it: refuse for read-only songs, detect external
//! changes against the fingerprint of the load or last save, back the
//! current on-disk file up once per session, then write atomically (temp
//! file in the same folder, fsync, rename, best-effort folder fsync). On
//! any error the original stays untouched and the temp file is removed. It
//! exists so that a save either fully replaces the file or changes nothing.

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use usdx_format::rows::Document;
use usdx_format::serialize::serialize;

use crate::controller::editor::LoadedSong;
use crate::controller::fingerprint::FileFingerprint;

/// The errors a save can fail with.
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    /// The song is read-only and may not be written.
    #[error("the song is read-only")]
    ReadOnly,
    /// The file on disk changed since the load or the last save; the save
    /// needs the user's confirmation.
    #[error("the file changed on disk since it was loaded")]
    ExternalChange,
    /// The document cannot be serialized.
    #[error("the song cannot be serialized: {0}")]
    Serialize(#[from] usdx_format::bytes::FormatError),
    /// A file operation failed.
    #[error("the song cannot be written: {0}")]
    Io(#[from] std::io::Error),
}

/// The save state of one session: which files already have their backup.
#[derive(Debug)]
pub struct SaveService {
    backups_written: HashSet<PathBuf>,
}

impl Default for SaveService {
    fn default() -> Self {
        Self::new()
    }
}

impl SaveService {
    /// new builds an empty save service.
    pub fn new() -> SaveService {
        SaveService {
            backups_written: HashSet::new(),
        }
    }

    /// save serializes the loaded document and writes it to `target`. When
    /// the file on disk changed since the load or last save, the save is
    /// refused with `ExternalChange` until the user confirms it.
    pub fn save(&mut self, loaded: &mut LoadedSong, target: &Path) -> Result<(), SaveError> {
        self.save_inner(loaded, target, real_serialize, false)
    }

    /// save_confirmed skips the external-change check; the bridge calls it
    /// after the user chose to overwrite the changed file.
    pub fn save_confirmed(
        &mut self,
        loaded: &mut LoadedSong,
        target: &Path,
    ) -> Result<(), SaveError> {
        self.save_inner(loaded, target, real_serialize, true)
    }

    /// save_with saves with a pluggable serializer; the tests inject a
    /// failing one to pin that a failure leaves the original file intact.
    pub fn save_with(
        &mut self,
        loaded: &mut LoadedSong,
        target: &Path,
        produce: impl FnOnce(&Document) -> Result<Vec<u8>, SaveError>,
        confirmed: bool,
    ) -> Result<(), SaveError> {
        self.save_inner(loaded, target, produce, confirmed)
    }

    /// save_inner runs the save steps in the plan's order.
    fn save_inner(
        &mut self,
        loaded: &mut LoadedSong,
        target: &Path,
        produce: impl FnOnce(&Document) -> Result<Vec<u8>, SaveError>,
        confirmed: bool,
    ) -> Result<(), SaveError> {
        if loaded.read_only {
            return Err(SaveError::ReadOnly);
        }
        if !confirmed && self.external_change(loaded, target)? {
            return Err(SaveError::ExternalChange);
        }
        // The first save of this session backs the current on-disk file up
        // once; a fresh path has nothing to back up.
        if target.exists() && self.backups_written.insert(target.to_owned()) {
            fs::copy(target, backup_path(target))?;
        }
        let bytes = produce(&loaded.document)?;
        write_atomic(target, &bytes)?;
        loaded.history.mark_saved();
        let metadata = fs::metadata(target).ok();
        loaded.fingerprint = FileFingerprint::new(
            metadata
                .as_ref()
                .map(|meta| meta.len())
                .unwrap_or(bytes.len() as u64),
            metadata.and_then(|meta| meta.modified().ok()),
            &bytes,
        );
        Ok(())
    }

    /// external_change tells whether the file on disk differs from the
    /// fingerprint of the load or last save. A missing file is not a change
    /// (the save creates it); an unreadable file is an error.
    fn external_change(&self, loaded: &LoadedSong, target: &Path) -> Result<bool, SaveError> {
        let bytes = match fs::read(target) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        let metadata = fs::metadata(target).ok();
        let current = FileFingerprint::new(
            metadata
                .as_ref()
                .map(|meta| meta.len())
                .unwrap_or(bytes.len() as u64),
            metadata.and_then(|meta| meta.modified().ok()),
            &bytes,
        );
        Ok(current != loaded.fingerprint)
    }
}

/// real_serialize serializes the document with the format's serializer.
fn real_serialize(document: &Document) -> Result<Vec<u8>, SaveError> {
    serialize(document).map_err(SaveError::Serialize)
}

/// backup_path returns the backup path of a target: the file name plus
/// `.bak` (plan section 8.7).
fn backup_path(target: &Path) -> PathBuf {
    let mut name = target
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    name.push(".bak");
    target.with_file_name(name)
}

/// write_atomic writes the bytes to a temp file in the target's folder,
/// fsyncs it and renames it over the target; the folder is fsynced
/// best-effort. On any error the target stays untouched and the temp file
/// is removed.
fn write_atomic(target: &Path, bytes: &[u8]) -> Result<(), SaveError> {
    let file_name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| std::io::Error::other("the target has no file name"))?;
    let temp = target.with_file_name(format!("{}.tmp-{}", file_name, std::process::id()));
    let result = write_temp(&temp, target, bytes);
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    Ok(result?)
}

/// write_temp performs the guarded part of the atomic write.
fn write_temp(temp: &Path, target: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let mut file = fs::File::create(temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(temp, target)?;
    if let Some(folder) = target.parent()
        && let Ok(dir) = fs::File::open(folder)
    {
        let _ = dir.sync_all();
    }
    Ok(())
}
