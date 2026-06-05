//! Windows-only implementation details. Kept as a sub-module so future
//! additions (registry probing, NT path manipulation, etc.) have an obvious
//! home that does not bleed into the rest of the crate.

pub mod exec;
