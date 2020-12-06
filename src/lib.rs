// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2 license.
//!
//! # HVCtrl
//! Hypervisor controller library
//!
//! # Supported OS
//! Windows only.
//!
//! # Supported hypervisor controller
//!
//! - VirtualBox
//!     - [VBoxManage](https://www.virtualbox.org/manual/ch08.html)
//! - VMWare Player
//!     - [VMRest](https://code.vmware.com/apis/413)
//! - Hyper-V
//!     - [Hyper-V cmdlets](https://docs.microsoft.com/en-us/powershell/module/hyper-v/?view=win10-ps)
//!
//! # License
//! This software is released under the MIT or Apache-2.0 License, see LICENSE-MIT or LICENSE-APACHE.
#[macro_use]
pub mod types;

pub mod hyperv;
pub mod virtualbox;
pub mod vmware;
