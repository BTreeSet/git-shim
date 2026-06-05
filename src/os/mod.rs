//! OS-segregated implementation details.
//!
//! All platform-specific behavior lives behind `#[cfg(...)]` here. The rest
//! of the crate must never branch on `cfg!()` at runtime for OS dispatch.

pub mod exec;
