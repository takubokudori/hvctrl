// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
//! VirtualBox controllers.

#[cfg(any(feature = "virtualbox", feature = "vboxmanage"))]
pub mod vboxmanage;

#[cfg(any(feature = "virtualbox", feature = "vboxmanage"))]
pub use vboxmanage::*;
