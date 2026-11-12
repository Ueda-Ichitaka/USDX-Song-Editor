//! About me: module root of the cxx-qt bridges. The bridges are thin Qt-facing
//! wrappers that convert between Qt and Rust types and forward to the plain-Rust
//! controllers; the module starts with the self-test helper bridge and grows as the
//! SongDocument, model, player and repair bridges are added in later tasks.

pub mod selftest;
