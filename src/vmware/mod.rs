// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
//! VMWare controllers.
pub mod vmrest;

#[cfg(any(feature = "vmware", feature = "vmrest"))]
pub use vmrest::*;
