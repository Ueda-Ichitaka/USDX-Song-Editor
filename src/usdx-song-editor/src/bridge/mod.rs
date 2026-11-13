//! About me: module root of the cxx-qt bridges. The bridges are thin Qt-facing
//! wrappers that convert between Qt and Rust types and forward to the plain-Rust
//! controllers; the module starts with the self-test helper bridge, the
//! SongDocument and LinesModel bridges and the playback logic of the Player
//! bridge; the sheet and repair bridges are added in later tasks.

pub mod player;
pub mod screenshot;
pub mod selftest;
pub mod song_document;
