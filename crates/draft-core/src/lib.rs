//! Platform-neutral core for Draft.
//!
//! This crate must not depend on a window system, an audio API, or a specific
//! operating system. Desktop and mobile shells own those details and call into
//! these modules.

pub mod text;
