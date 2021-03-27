// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
use serde::{Deserialize, Serialize};
use std::process::Command;
use windy::AString;

/// Executes `cmd` and Returns `(stdout, stderr)`.
pub(crate) fn exec_cmd(cmd: &mut Command) -> VmResult<(String, String)> {
    match cmd.output() {
        Ok(o) => unsafe {
            Ok((
                AString::new_unchecked(o.stdout).to_string_lossy(),
                AString::new_unchecked(o.stderr).to_string_lossy(),
            ))
        },
        Err(x) => Err(VmError::from(ErrorKind::ExecutionFailed(x.to_string()))),
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub struct VmError {
    repr: Repr,
}

impl std::error::Error for VmError {}

impl std::fmt::Display for VmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        "test".fmt(f)
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum Repr {
    Simple(ErrorKind),
    Unknown(String),
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum ErrorKind {
    AuthenticationFailed,
    ExecutionFailed(String),
    FileError(String),
    GuestAuthenticationFailed,
    InvalidParameter(String),
    InvalidVmState,
    NetworkAdaptorNotFound,
    NetworkNotFound,
    /// Requires any privileges to control a VM.
    PrivilegesRequired,
    SnapshotNotFound,
    UnexpectedResponse(String),
    UnsupportedCommand,
    VmIsNotRunning,
    VmIsRunning,
    VmNotFound,
}

impl From<Repr> for VmError {
    fn from(repr: Repr) -> Self {
        Self { repr }
    }
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
}

macro_rules! starts_err {
    ($s:expr, $x:expr, $y:expr) => {
        if $s.starts_with($x) {
            return $crate::types::VmError::from($y);
        }
    };
}

pub trait PowerCmd {
    /// Starts a VM and waits for the VM to start.
    fn start(&self) -> VmResult<()>;
    /// Stops a VM softly and waits for the VM to stop.
    fn stop(&self) -> VmResult<()>;
    /// Stops a VM hardly and waits for the VM to stop.
    fn hard_stop(&self) -> VmResult<()>;
    /// Suspends a VM and waits for the VM to suspend.
    fn suspend(&self) -> VmResult<()>;
    /// Resumes a VM and waits for the VM to start.
    fn resume(&self) -> VmResult<()>;
    /// Returns `true` if a VM is running.
    fn is_running(&self) -> VmResult<bool>;
    /// Reboots a VM softly and waits for the VM to start.
    fn reboot(&self) -> VmResult<()>;
    /// Reboots a VM hardly and waits for the VM to start.
    fn hard_reboot(&self) -> VmResult<()>;
    /// Pauses a VM and waits for the VM to pause.
    fn pause(&self) -> VmResult<()>;
    /// Unpauses a VM and waits for the VM to unpause.
    fn unpause(&self) -> VmResult<()>;
}

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

pub trait GuestCmd {
    /// Runs a command in guest.
    fn run_command(&self, guest_args: &[&str]) -> VmResult<()>;
    /// Copies a file from a guest to a host.
    fn copy_from_guest_to_host(&self, from_guest_path: &str, to_host_path: &str) -> VmResult<()>;
    /// Copies a file from a host to a guest.
    fn copy_from_host_to_guest(&self, from_host_path: &str, to_guest_path: &str) -> VmResult<()>;
}

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
    pub id: Option<String>,
    pub name: Option<String>,
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
#[derive(Debug, Eq, PartialEq)]
pub enum VmPowerState {
    Running,
    Stopped,
    Suspended,
    Paused,
    Unknown,
}

impl VmPowerState {
    #[inline]
    pub fn is_running(&self) -> bool {
        *self == Self::Running
    }
}
