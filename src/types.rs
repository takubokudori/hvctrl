// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
#![allow(dead_code)]
#![allow(unused_macros)]
use crate::vmerr;
use serde::{Deserialize, Serialize};
use std::{process::Command, time::Duration};

use std::string::FromUtf8Error;
#[cfg(windows)]
use windy::AString;

#[cfg(windows)]
/// Executes `cmd` and Returns `(stdout, stderr)`.
pub(crate) fn exec_cmd_astr(cmd: &mut Command) -> VmResult<(String, String)> {
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

/// Executes `cmd` and Returns `(stdout, stderr)`.
pub(crate) fn exec_cmd_utf8(cmd: &mut Command) -> VmResult<(String, String)> {
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

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct VmError {
    repr: Repr,
}

impl std::error::Error for VmError {}

impl std::fmt::Display for VmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        format!("{:?}", self).fmt(f)
    }
}

impl VmError {
    pub fn get_invalid_state(&self) -> Option<VmPowerState> {
        match &self.repr {
            Repr::Simple(ErrorKind::InvalidPowerState(x)) => Some(*x),
            _ => None,
        }
    }

    pub fn get_repr(&self) -> &Repr { &self.repr }

    pub fn is_invalid_state_running(&self) -> Option<bool> {
        self.get_invalid_state().map(|x| x.is_running())
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Repr {
    Simple(ErrorKind),
    Unknown(String),
    SerializeError,
    IoError,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum ErrorKind {
    AuthenticationFailed,
    ExecutionFailed(String),
    FileError(String),
    GuestAuthenticationFailed,
    GuestFileNotFound,
    GuestFileExists,
    HostFileNotFound,
    HostFileExists,
    InvalidParameter(String),
    /// InvalidPowerState contains the current VM power state.
    InvalidPowerState(VmPowerState),
    FromUtf8Error(FromUtf8Error),
    NetworkAdaptorNotFound,
    NetworkNotFound,
    /// Requires any privileges to control a VM.
    PrivilegesRequired,
    /// The guest service (e.g., [VirtualBox Guest Additions](https://www.virtualbox.org/manual/ch04.html#guestadd-intro), [VMware Tools](https://docs.vmware.com/en/VMware-Tools/index.html), [Hyper-V Integration Service](https://docs.microsoft.com/en-us/virtualization/hyper-v-on-windows/reference/integration-services), etc...) that controls a VM is not running, ready or installed.
    ServiceIsNotRunning,
    SnapshotNotFound,
    SnapshotExists,
    /// The specified action was not completed in time.
    Timeout,
    UnexpectedResponse(String),
    UnsupportedCommand,
    VmIsNotSpecified,
    VmNotFound,
}

impl From<Repr> for VmError {
    fn from(repr: Repr) -> Self { Self { repr } }
}

impl From<std::io::Error> for VmError {
    fn from(_: std::io::Error) -> Self { vmerr!(@r Repr::IoError) }
}

impl From<serde_json::Error> for VmError {
    fn from(_: serde_json::Error) -> Self { vmerr!(@r Repr::SerializeError) }
}

impl From<ErrorKind> for VmError {
    fn from(e: ErrorKind) -> Self {
        Self {
            repr: Repr::Simple(e),
        }
    }
}

pub type VmResult<T> = Result<T, VmError>;

#[macro_export]
macro_rules! vmerr {
    ($x:expr) => {
        Err($crate::types::VmError::from($x))
    };
    (@r $x:expr) => {
        $crate::types::VmError::from($x)
    };
}

macro_rules! starts_err {
    ($s:expr, $x:expr, $y:expr) => {
        if $s.starts_with($x) {
            return $crate::types::VmError::from($y);
        }
    };
}

/// A trait for a VM information.
pub trait VmCmd {
    /// Get a list of VMs.
    fn list_vms(&self) -> VmResult<Vec<Vm>>;
    /// Sets the VM specified by the `id` of the VM.
    /// If the corresponding VM doesn't exist, return [`ErrorKind::VmNotFound`].
    ///
    /// The ID type depends on the tool you are using.
    fn set_vm_by_id(&mut self, id: &str) -> VmResult<()>;
    /// Sets the VM specified by the `name` of the VM.
    /// If the corresponding VM doesn't exist, return [`ErrorKind::VmNotFound`].
    fn set_vm_by_name(&mut self, name: &str) -> VmResult<()>;
    /// Sets the VM specified by the `path` of the VM file.
    /// If the corresponding VM doesn't exist, return [`ErrorKind::VmNotFound`].
    ///
    /// The file type depends on the tool you are using.
    fn set_vm_by_path(&mut self, path: &str) -> VmResult<()>;
}

/// A trait for managing power state of a VM.
pub trait PowerCmd {
    /// Starts the VM and waits for the VM to start.
    fn start(&self) -> VmResult<()>;
    /// Stops the VM softly and waits for the VM to stop.
    ///
    /// This function usually only sends a ACPI shutdown signal, so there is no guarantee that calling this function will shut down the VM.
    fn stop<D: Into<Option<Duration>>>(&self, timeout: D) -> VmResult<()>;
    /// Stops the VM hardly and waits for the VM to stop.
    fn hard_stop(&self) -> VmResult<()>;
    /// Suspends the VM and waits for the VM to suspend.
    fn suspend(&self) -> VmResult<()>;
    /// Resumes the suspended VM.
    fn resume(&self) -> VmResult<()>;
    /// Returns `true` if the VM is running.
    fn is_running(&self) -> VmResult<bool>;
    /// Reboots the VM softly and waits for the VM to start.
    fn reboot<D: Into<Option<Duration>>>(&self, timeout: D) -> VmResult<()>;
    /// Reboots the VM hardly and waits for the VM to start.
    fn hard_reboot(&self) -> VmResult<()>;
    /// Pauses the VM and waits for the VM to pause.
    fn pause(&self) -> VmResult<()>;
    /// Unpauses the VM and waits for the VM to unpause.
    fn unpause(&self) -> VmResult<()>;
}

/// A trait for managing snapshots of a VM.
pub trait SnapshotCmd {
    /// Returns snapshots of a VM.
    fn list_snapshots(&self) -> VmResult<Vec<Snapshot>>;
    /// Takes a snapshot of a VM.
    fn take_snapshot(&self, name: &str) -> VmResult<()>;
    /// Reverts the current VM state to a snapshot of the VM.
    fn revert_snapshot(&self, name: &str) -> VmResult<()>;
    /// Deletes a snapshot of a VM.
    fn delete_snapshot(&self, name: &str) -> VmResult<()>;
}

/// A trait for controlling a guest OS.
pub trait GuestCmd {
    /// Executes a command on guest.
    fn exec_cmd(&self, guest_args: &[&str]) -> VmResult<()>;
    /// Copies a file from a guest to a host.
    fn copy_from_guest_to_host(
        &self,
        from_guest_path: &str,
        to_host_path: &str,
    ) -> VmResult<()>;
    /// Copies a file from a host to a guest.
    fn copy_from_host_to_guest(
        &self,
        from_host_path: &str,
        to_guest_path: &str,
    ) -> VmResult<()>;
}

/// A trait for managing NICs of a VM.
pub trait NicCmd {
    /// Returns NICs of a VM.
    fn list_nics(&self) -> VmResult<Vec<Nic>>;
    /// Adds a NIC to a VM.
    fn add_nic(&self, nic: &Nic) -> VmResult<()>;
    /// Updates a NIC.
    fn update_nic(&self, nic: &Nic) -> VmResult<()>;
    /// Removes a NIC from a VM.
    fn remove_nic(&self, nic: &Nic) -> VmResult<()>;
}

/// A trait for managing shared folders of a VM.
pub trait SharedFolderCmd {
    /// Returns shared folders of a VM.
    fn list_shared_folders(&self) -> VmResult<Vec<SharedFolder>>;
    /// Mounts a shared folder to a VM.
    fn mount_shared_folder(&self, shfs: &SharedFolder) -> VmResult<()>;
    /// Unmounts a shared folder to a VM.
    fn unmount_shared_folder(&self, shfs: &SharedFolder) -> VmResult<()>;
    /// Deletes a snapshot of a VM.
    fn delete_shared_folder(&self, shfs: &SharedFolder) -> VmResult<()>;
}

/// Represents a VM information.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Vm {
    /// Unique ID for the VM.
    pub id: Option<String>,
    /// The name of the VM.
    pub name: Option<String>,
    /// The path to the VM file.
    pub path: Option<String>,
}

impl PartialEq for Vm {
    fn eq(&self, other: &Self) -> bool {
        if let (Some(x), Some(x2)) = (&self.id, &other.id) {
            return x == x2;
        }
        if let (Some(x), Some(x2)) = (&self.path, &other.path) {
            return x == x2;
        }
        if let (Some(x), Some(x2)) = (&self.name, &other.name) {
            return x == x2;
        }
        false
    }
}

/// Represents a snapshot of a VM.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Snapshot {
    pub id: Option<String>,
    pub name: Option<String>,
    pub detail: Option<String>,
}

impl PartialEq for Snapshot {
    fn eq(&self, other: &Self) -> bool {
        if let (Some(x), Some(x2)) = (&self.id, &other.id) {
            return x == x2;
        }
        if let (Some(x), Some(x2)) = (&self.name, &other.name) {
            return x == x2;
        }
        false
    }
}

/// Represents a NIC type.
#[derive(Debug, Eq, PartialEq, Clone, Hash, Serialize, Deserialize)]
pub enum NicType {
    Bridge,
    #[allow(clippy::upper_case_acronyms)]
    NAT,
    HostOnly,
    Custom(String),
}

/// Represents a NIC.
#[derive(Debug, Clone, Serialize, Deserialize, Default, Hash)]
pub struct Nic {
    pub id: Option<String>,
    pub name: Option<String>,
    pub ty: Option<NicType>,
    pub mac_address: Option<String>,
}

/// Represents a shared folder.
#[derive(Debug, Clone, Serialize, Deserialize, Default, Hash)]
pub struct SharedFolder {
    pub id: Option<String>,
    pub name: Option<String>,
    pub guest_path: Option<String>,
    pub host_path: Option<String>,
    pub is_readonly: bool,
}

/// Represents a VM power state.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum VmPowerState {
    /// The VM is running.
    Running,
    /// The VM is not running.
    ///
    /// This state contains `Stopped`, `Suspended` and `Paused`.
    NotRunning,
    /// The VM is stopped.
    Stopped,
    /// The VM is suspended.
    Suspended,
    /// The VM is paused.
    Paused,
    /// The VM is in an unknown state.
    ///
    /// Due to the specifications of the tool you are using, it may not be able to detect the VM state accurately.
    Unknown,
}

impl VmPowerState {
    #[inline]
    pub fn is_running(&self) -> bool { *self == Self::Running }
}

macro_rules! impl_setter {
    ($(#[$inner:meta])* $name:ident : $t:ty) => {
        $(#[$inner])*
        pub fn $name<T: Into<$t>>(&mut self, $name: T) -> &mut Self {
            self.$name = $name.into();
            self
        }
    };
    (@opt $(#[$inner:meta])* $name:ident : $t:ty) => {
        $(#[$inner])*
        pub fn $name<T: Into<Option<$t>>>(&mut self, $name: T) -> &mut Self {
            self.$name = $name.into();
            self
        }
    };
}
