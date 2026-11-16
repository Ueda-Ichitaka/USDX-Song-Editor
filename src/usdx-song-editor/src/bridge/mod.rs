//! About me: module root of the cxx-qt bridges. The bridges are thin Qt-facing
//! wrappers that convert between Qt and Rust types and forward to the plain-Rust
//! controllers; the module starts with the self-test helper bridge, the
//! SongDocument and LinesModel bridges, the playback logic of the Player
//! bridge, the painted SheetBackground of the note sheet and the pure
//! logic of the Repair and TapSession bridges.

pub mod player;
pub mod repair;
pub mod screenshot;
pub mod selftest;
pub mod sheet_background;
pub mod song_document;
