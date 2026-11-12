//! About me: integration test of the usdx-audio library crate. It fails while the
//! library does not exist and passes once the crate is part of the workspace and
//! builds, so the workspace skeleton task (P0-01) is red before and green after the
//! crate exists.

use usdx_audio as _;

#[test]
fn crate_builds() {}
