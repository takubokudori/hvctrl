use serde::{Serialize, Deserialize};
use std::process::Command;
use winwrap::string::AString;

/// Executes `cmd` and Returns `(stdout, stderr)`.
pub(crate) fn exec_cmd(cmd: &mut Command) -> VMResult<(String, String)> {
    match cmd.output() {
        Ok(o) => {
            Ok((AString::new(o.stdout).to_string_lossy(), AString::new(o.stderr).to_string_lossy()))
        }
        Err(x) => Err(VMError::from(ErrorKind::ExecutionFailed(x.to_string())))
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub struct VMError {
    repr: Repr,
}

impl std::error::Error for VMError {}

impl std::fmt::Display for VMError {
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
    GuestAuthenticationFailed,
    ExecutionFailed(String),
    FileError(String),
    InvalidParameter(String),
    SnapshotNotFound,
    UnexpectedResponse(String),
    UnsupportedCommand,
    VMIsNotRunning,
    VMIsRunning,
    VMNotFound,
    NetworkNotFound,
    NetworkAdaptorNotFound,
    /// Requires any privileges to control a VM.
    PrivilegesRequired,
}

impl From<Repr> for VMError {
    fn from(repr: Repr) -> Self {
        Self { repr }
    }
}

impl From<ErrorKind> for VMError {
    fn from(e: ErrorKind) -> Self {
        Self { repr: Repr::Simple(e) }
    }
}

pub type VMResult<T> = Result<T, VMError>;

#[macro_export]
macro_rules! vmerr {
    ($x:expr) => { Err(crate::types::VMError::from($x)) }
}

macro_rules! starts_err {
    ($s:expr, $x:expr, $y:expr) => {
        if $s.starts_with($x) { return crate::types::VMError::from($y); }
     }
}

pub trait PowerCmd {
    /// Starts a VM and waits for the VM to start.
    fn start(&self) -> VMResult<()>;
    /// Stops a VM softly and waits for the VM to stop.
    fn stop(&self) -> VMResult<()>;
    /// Stops a VM hardly and waits for the VM to stop.
    fn hard_stop(&self) -> VMResult<()>;
    /// Suspends a VM and waits for the VM to suspend.
    fn suspend(&self) -> VMResult<()>;
    /// Resumes a VM and waits for the VM to start.
    fn resume(&self) -> VMResult<()>;
    fn is_running(&self) -> VMResult<bool>;
    /// Reboots a VM softly and waits for the VM to start.
    fn reboot(&self) -> VMResult<()>;
    /// Reboots a VM hardly and waits for the VM to start.
    fn hard_reboot(&self) -> VMResult<()>;
    /// Pauses a VM and waits for the VM to pause.
    fn pause(&self) -> VMResult<()>;
    /// Unpauses a VM and waits for the VM to unpause.
    fn unpause(&self) -> VMResult<()>;
}

pub trait SnapshotCmd {
    /// Returns snapshots of a VM.
    fn list_snapshots(&self) -> VMResult<Vec<Snapshot>>;
    /// Takes a snapshot of a VM.
    fn take_snapshot(&self, name: &str) -> VMResult<()>;
    /// Reverts the current VM state to a snapshot of the VM.
    fn revert_snapshot(&self, name: &str) -> VMResult<()>;
    /// Deletes a snapshot of a VM.
    fn delete_snapshot(&self, name: &str) -> VMResult<()>;
}

pub trait GuestCmd {
    /// Runs a command in guest.
    fn run_command(&self, guest_args: &[&str]) -> VMResult<()>;
    /// Copies a file from a guest to a host.
    fn copy_from_guest_to_host(&self, from_guest_path: &str, to_host_path: &str) -> VMResult<()>;
    /// Copies a file from a host to a guest.
    fn copy_from_host_to_guest(&self, from_host_path: &str, to_guest_path: &str) -> VMResult<()>;
}

pub trait NICCmd {
    /// Returns NICs of a VM.
    fn list_nics(&self) -> VMResult<Vec<NIC>>;
    fn add_nic(&self, nic: &NIC) -> VMResult<()>;
    fn update_nic(&self, nic: &NIC) -> VMResult<()>;
    fn remove_nic(&self, nic: &NIC) -> VMResult<()>;
}

pub trait SharedFolderCmd {
    /// Returns shared folders of a VM.
    fn list_shared_folders(&self) -> VMResult<Vec<SharedFolder>>;
    /// Mounts a shared folder to a VM.
    fn mount_shared_folder(&self, shfs: &SharedFolder) -> VMResult<()>;
    /// Unmounts a shared folder to a VM.
    fn unmount_shared_folder(&self, shfs: &SharedFolder) -> VMResult<()>;
    /// Deletes a snapshot of a VM.
    fn delete_shared_folder(&self, shfs: &SharedFolder) -> VMResult<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, Hash)]
pub struct VM {
    pub id: Option<String>,
    pub name: Option<String>,
    pub path: Option<String>,
}

impl PartialEq for VM {
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

#[derive(Debug, Clone, Serialize, Deserialize, Default, Hash)]
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

/// Represents NIC type.
#[derive(Debug, Eq, PartialEq, Clone, Hash, Serialize, Deserialize)]
pub enum NICType {
    Bridge,
    NAT,
    HostOnly,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, Hash)]
pub struct NIC {
    pub id: Option<String>,
    pub name: Option<String>,
    pub ty: Option<NICType>,
    pub mac_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, Hash)]
pub struct SharedFolder {
    pub id: Option<String>,
    pub name: Option<String>,
    pub guest_path: Option<String>,
    pub host_path: Option<String>,
    pub is_readonly: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub enum VMPowerState {
    Running,
    Stopped,
    Suspended,
    Paused,
    Unknown,
}

impl VMPowerState {
    #[inline]
    pub fn is_running(&self) -> bool { *self == Self::Running }
}
