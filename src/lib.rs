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

#[macro_use]
extern crate log;

use crate::types::{ErrorKind, VmError, VmResult};
use log::Level;
use serde::Deserialize;
use std::{io::Write, process::Command};
#[cfg(windows)]
use windy::AString;

#[allow(dead_code)]
pub(crate) fn deserialize<'a, T: Deserialize<'a>>(s: &'a str) -> VmResult<T> {
    serde_json::from_str(s)
        .map_err(|x| vmerr!(@r ErrorKind::UnexpectedResponse(x.to_string())))
}

#[cfg(windows)]
#[allow(dead_code)]
pub(crate) fn exec_cmd_astr(cmd: &mut Command) -> VmResult<(String, String)> {
    dbg_cmd(cmd);
    match cmd.output() {
        Ok(o) => unsafe {
            Ok((
                AString::new_unchecked(o.stdout).to_string_lossy(),
                AString::new_unchecked(o.stderr).to_string_lossy(),
            ))
        },
        Err(x) => vmerr!(ErrorKind::ExecutionFailed(x.to_string())),
    }
}

#[allow(dead_code)]
pub(crate) fn exec_cmd(cmd: &mut Command) -> VmResult<(String, String)> {
    #[cfg(windows)]
    {
        exec_cmd_astr(cmd)
    }
    #[cfg(not(windows))]
    {
        exec_cmd_utf8(cmd)
    }
}

#[allow(dead_code)]
/// Executes `cmd` and Returns `(stdout, stderr)`.
pub(crate) fn exec_cmd_utf8(cmd: &mut Command) -> VmResult<(String, String)> {
    dbg_cmd(cmd);
    match cmd.output() {
        Ok(o) => Ok((
            String::from_utf8(o.stdout)
                .map_err(|e| VmError::from(ErrorKind::FromUtf8Error(e)))?,
            String::from_utf8(o.stderr)
                .map_err(|e| VmError::from(ErrorKind::FromUtf8Error(e)))?,
        )),
        Err(x) => vmerr!(ErrorKind::ExecutionFailed(x.to_string())),
    }
}

#[allow(dead_code)]
pub(crate) fn dbg_cmd(cmd: &Command) {
    if log_enabled!(Level::Debug) {
        let args = cmd.get_args();
        let stdout = std::io::stdout();
        let mut stdout = stdout.lock();
        write!(stdout, "{}", cmd.get_program().to_str().unwrap()).unwrap();
        for arg in args {
            write!(stdout, " {}", arg.to_str().unwrap()).unwrap();
        }
        writeln!(stdout).unwrap();
        stdout.flush().unwrap();
    }
}
