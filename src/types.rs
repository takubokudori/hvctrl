use serde::{Serialize, Deserialize};

#[derive(Debug, Eq, PartialEq)]
pub struct VMError {
    repr: Repr,
}

impl std::error::Error for VMError {}

impl std::fmt::Display for VMError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        "test".fmt(f)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Repr {
    Simple(ErrorKind),
    Unknown(String),
}

#[derive(Debug, Eq, PartialEq)]
pub enum ErrorKind {
    AuthenticationFailed,
    ExecutionFailed(String),
    FileError(String),
    InvalidParameter(String),
    SnapshotNotFound,
    UnexpectedResponse(String),
    UnsupportedCommand,
    VMIsNotPoweredOn,
    VMIsPoweredOn,
    VMNotFound,
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

pub trait PowerCmd {
    fn start(&self) -> VMResult<()>;
    fn stop(&self) -> VMResult<()>;
    fn hard_stop(&self) -> VMResult<()>;
    fn suspend(&self) -> VMResult<()>;
    fn resume(&self) -> VMResult<()>;
    fn is_running(&self) -> VMResult<bool>;
    fn reboot(&self) -> VMResult<()>;
    fn hard_reboot(&self) -> VMResult<()>;
    fn pause(&self) -> VMResult<()>;
    fn unpause(&self) -> VMResult<()>;
}

pub trait SnapshotCmd {
    fn list_snapshots(&mut self) -> VMResult<Vec<Snapshot>>;
    fn take_snapshot(&mut self, name: &str) -> VMResult<()>;
    fn revert_snapshot(&mut self, name: &str) -> VMResult<()>;
    fn delete_snapshot(&mut self, name: &str) -> VMResult<()>;
}

pub trait GuestCmd {
    fn run_command(&self, guest_args: &[&str]) -> VMResult<()>;
    fn copy_from_guest_to_host(&mut self, from_guest_path: &str, to_host_path: &str) -> VMResult<()>;
    fn copy_from_host_to_guest(&mut self, from_host_path: &str, to_guest_path: &str) -> VMResult<()>;
}

pub trait NICCmd {
    fn list_nics(&self) -> VMResult<Vec<NIC>>;
}

pub trait SharedFolderCmd {
    fn list_shared_folders(&self) -> VMResult<Vec<SharedFolder>>;
    fn mount_shared_folder(&self) -> VMResult<()>;
    fn unmount_shared_folder(&self) -> VMResult<()>;
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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
#[derive(Debug, Eq, PartialEq)]
pub enum NICType {
    Bridge,
    NAT,
    HostOnly,
    Custom,
}

#[derive(Debug)]
pub struct NIC {
    pub id: Option<String>,
    pub name: Option<String>,
    pub ty: Option<NICType>,
    pub mac_address: Option<String>,
}

#[derive(Debug)]
pub struct SharedFolder {
    pub id: Option<String>,
    pub name: Option<String>,
    pub guest_path: Option<String>,
    pub host_path: Option<String>,
    pub is_readonly: bool,
}
