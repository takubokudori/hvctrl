// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
//! Hyper-V controllers.
#![cfg(windows)]
#[cfg(feature = "hypervcmd")]
pub mod hypervcmd;

#[cfg(feature = "hypervcmd")]
pub use hypervcmd::*;
