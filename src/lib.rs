// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
//! # HvCtrl
//! A hypervisor controller library
//!
//! # Supported OS
//! Windows only.
//!
//! # Supported hypervisor controller
//!
//! - [VirtualBox](https://www.virtualbox.org/)
//!     - [VBoxManage](https://www.virtualbox.org/manual/ch08.html)
//! - [VMWare Workstation Player](https://www.vmware.com/products/workstation-player.html)
//!     - [VMRest](https://code.vmware.com/apis/413)
//! - [Hyper-V](https://docs.microsoft.com/en-us/virtualization/hyper-v-on-windows/about/)
//!     - [Hyper-V cmdlets](https://docs.microsoft.com/en-us/powershell/module/hyper-v/?view=win10-ps)
//!
//! # License
//!
//! This software is released under the MIT or Apache-2.0 License, see LICENSE-MIT or LICENSE-APACHE.
#[macro_use]
pub mod types;

pub mod hyperv;
pub mod virtualbox;
pub mod vmware;

use crate::types::{ErrorKind, VmResult};
use serde::Deserialize;

#[allow(dead_code)]
pub(crate) fn deserialize<'a, T: Deserialize<'a>>(s: &'a str) -> VmResult<T> {
    serde_json::from_str(s)
        .map_err(|x| vmerr!(@r ErrorKind::UnexpectedResponse(x.to_string())))
}
