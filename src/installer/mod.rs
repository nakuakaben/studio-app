//! Explicit approved installations; no update, uninstall, discovery or shell.
pub mod approval;
pub mod archive;
pub mod download;
#[cfg(feature = "installer-fixtures")]
pub mod fixture;
pub mod model;
pub mod personal;
mod personal_native;
pub mod platform;
#[allow(dead_code)]
// Staged validator: activate only after a separately approved portable release exists.
mod portable_manifest;
pub mod worker;
pub use model::{Cancellation, Failure, Phase, Progress, Result};

#[cfg(test)]
mod tests;
