use crate::{types::*, vmware::read_vmware_preferences};
use std::{process::Command, time::Duration};

pub enum HostType {
    Player,
    Workstation,
    Fusion,
}

impl HostType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Player => "player",
            Self::Workstation => "ws",
            Self::Fusion => "fusion",
        }
    }
}

impl ToString for HostType {
    fn to_string(&self) -> String { self.as_str().to_string() }
}

impl<T: AsRef<str>> From<T> for HostType {
    fn from(x: T) -> Self {
        match x.as_ref() {
            "player" => Self::Player,
            "ws" => Self::Workstation,
            "fusion" => Self::Fusion,
            x => panic!("Unexpected HostType: {}", x),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VmRun {
    host_type: &'static str,
    executable_path: String,
    vm_path: Option<String>,
    vm_password: Option<String>,
    guest_username: Option<String>,
    guest_password: Option<String>,
    gui: bool,
}

impl Default for VmRun {
    fn default() -> Self { Self::new() }
}

impl VmRun {
    pub fn new() -> Self {
        Self {
            host_type: "ws",
            executable_path: "vmrun".to_string(),
            vm_path: None,
            vm_password: None,
            guest_username: None,
            guest_password: None,
            gui: true,
        }
    }

    impl_setter!(
        /// Sets the path to vmrun.
        executable_path: String
    );

    pub fn host_type<T: Into<HostType>>(&mut self, host_type: T) -> &mut Self {
        self.host_type = host_type.into().as_str();
        self
    }

    impl_setter!(@opt vm_path: String);
    impl_setter!(@opt vm_password: String);
    impl_setter!(@opt guest_username: String);
    impl_setter!(@opt guest_password: String);
    impl_setter!(gui: bool);

    #[inline]
    fn build_auth(&self) -> Vec<&str> {
        let mut v = Vec::with_capacity(6);
        if let Some(x) = &self.guest_username {
            v.extend(&["-gu", x]);
        }
        if let Some(x) = &self.guest_password {
            v.extend(&["-gp", x]);
        }
        if let Some(x) = &self.vm_password {
            v.extend(&["-vp", x]);
        }
        v
    }

    fn get_vm(&self) -> VmResult<&str> {
        self.vm_path
            .as_deref()
            .ok_or_else(|| VmError::from(ErrorKind::VmIsNotSpecified))
    }

    #[inline]
    fn cmd(&self) -> Command {
        let mut cmd = Command::new(&self.executable_path);
        cmd.args(&["-T", self.host_type]);
        cmd.args(&self.build_auth());
        cmd
    }

    #[inline]
    fn handle_error(s: &str) -> VmError {
        use ErrorKind::*;
        use VmPowerState::*;
        starts_err!(s, "No Vm name provided", VmIsNotSpecified);
        starts_err!(s, "Cannot open VM: ", VmNotFound);
        starts_err!(
            s,
            "The virtual machine is not powered on: ",
            InvalidPowerState(NotRunning)
        );
        starts_err!(
            s,
            "A snapshot with the name already exists",
            SnapshotExists
        );
        starts_err!(
            s,
            "Invalid user name or password for the guest OS",
            AuthenticationFailed
        );
        starts_err!(s, "Unrecognized command: ", UnsupportedCommand);
        VmError::from(Repr::Unknown(format!("Unknown error: {}", s)))
    }

    #[inline]
    fn check(s: String) -> VmResult<String> {
        match s.strip_prefix("Error: ") {
            Some(s) => Err(Self::handle_error(s.trim())),
            None => Ok(s),
        }
    }

    fn exec(cmd: &mut Command) -> VmResult<String> {
        let (stdout, stderr) = exec_cmd(cmd)?;
        if !stderr.is_empty() {
            Self::check(stderr)
        } else {
            Self::check(stdout)
        }
    }

    /// Gets vmrun version, e.g., `vmrun version 1.17.0 build-17801498`.
    pub fn version(&self) -> VmResult<String> {
        let s = Self::exec(&mut self.cmd())?;
        let v = s
            .lines()
            .nth(2)
            .unwrap()
            .strip_prefix("vmrun version ")
            .unwrap();
        Ok(v.to_string())
    }

    pub fn start_vm(&self, gui: bool) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["start", self.get_vm()?]);
        if !gui {
            cmd.arg("nogui");
        }
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn stop_vm(&self, hard_stop: Option<bool>) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["stop", self.get_vm()?]);
        if let Some(hard_stop) = hard_stop {
            cmd.arg(if hard_stop { "soft" } else { "hard" });
        }
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn reset_vm(&self, hard_stop: Option<bool>) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["reset", self.get_vm()?]);
        if let Some(hard_stop) = hard_stop {
            cmd.arg(if hard_stop { "soft" } else { "hard" });
        }
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn suspend_vm(&self, hard_stop: Option<bool>) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["suspend", self.get_vm()?]);
        if let Some(hard_stop) = hard_stop {
            cmd.arg(if hard_stop { "soft" } else { "hard" });
        }
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn pause_vm(&self) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["pause", self.get_vm()?]);
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn unpause_vm(&self) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["unpause", self.get_vm()?]);
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn list_all_vms(&self) -> VmResult<Vec<Vm>> {
        let p = std::env::var("APPDATA").expect("Failed to get %APPDATA%");
        let vms =
            read_vmware_preferences(&format!(r"{}\VMware\preferences.ini", p))?;
        if vms.is_none() {
            return vmerr!(Repr::Unknown(
                "Cannot parse preferences file".to_string()
            ));
        }
        Ok(vms.unwrap())
    }

    pub fn list_running_vms(&self) -> VmResult<Vec<Vm>> {
        let mut cmd = self.cmd();
        cmd.arg("list");
        let s = Self::exec(&mut cmd)?;
        let mut l = s.lines();
        let n = match l.next() {
            Some(s) => s
                .strip_prefix("Total running VMs: ")
                .expect("Unexpected list response")
                .parse::<usize>()
                .expect("Failed to parse to usize"),
            None => return Ok(vec![]),
        };
        let mut ret = Vec::with_capacity(n);
        for s in l {
            ret.push(Vm {
                id: None,
                name: None,
                path: Some(s.to_string()),
            });
        }
        Ok(ret)
    }

    pub fn list_snapshots(&self) -> VmResult<Vec<Snapshot>> {
        let mut cmd = self.cmd();
        cmd.args(&["listSnapshots", self.get_vm()?]);
        let s = Self::exec(&mut cmd)?;
        let mut l = s.lines();
        let n = match l.next() {
            Some(s) => s
                .strip_prefix("Total snapshots: ")
                .expect("Unexpected list response")
                .parse::<usize>()
                .expect("Failed to parse to usize"),
            None => return Ok(vec![]),
        };
        let mut ret = Vec::with_capacity(n);
        for s in l {
            ret.push(Snapshot {
                id: None,
                name: Some(s.to_string()),
                detail: None,
            });
        }
        Ok(ret)
    }

    pub fn is_snapshot_exists(&self, name: &str) -> VmResult<bool> {
        let ss = self.list_snapshots()?;
        Ok(ss.iter().any(|x| x.name.as_deref().unwrap() == name))
    }

    pub fn snapshot(&self, name: &str) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["snapshot", self.get_vm()?, name]);
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn delete_snapshot(
        &self,
        name: &str,
        delete_children: bool,
    ) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["deleteSnapshot", self.get_vm()?, name]);
        if delete_children {
            cmd.arg("andDeleteChildren");
        }
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn revert_to_snapshot(&self, name: &str) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["revertToSnapshot", self.get_vm()?, name]);
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn run_program_in_guest(
        &self,
        no_wait: bool,
        active_window: bool,
        interactive: bool,
        program_args: &[&str],
    ) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["runProgramInGuest", self.get_vm()?]);
        if no_wait {
            cmd.arg("-noWait");
        }
        if active_window {
            cmd.arg("-activeWindow");
        }
        if interactive {
            cmd.arg("-interactive");
        }
        cmd.args(program_args);
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn install_tools(&self) -> VmResult<()> {
        Self::exec(self.cmd().args(&["installTools", self.get_vm()?]))?;
        Ok(())
    }

    pub fn copy_file_from_host_to_guest(
        &self,
        host_path: &str,
        guest_path: &str,
    ) -> VmResult<()> {
        Self::exec(self.cmd().args(&[
            "CopyFileFromHostToGuest",
            self.get_vm()?,
            host_path,
            guest_path,
        ]))?;
        Ok(())
    }

    pub fn copy_file_from_guest_to_host(
        &self,
        guest_path: &str,
        host_path: &str,
    ) -> VmResult<()> {
        Self::exec(self.cmd().args(&[
            "CopyFileFromGuestToHost",
            self.get_vm()?,
            guest_path,
            host_path,
        ]))?;
        Ok(())
    }
}

impl VmCmd for VmRun {
    fn list_vms(&self) -> VmResult<Vec<Vm>> { self.list_all_vms() }

    /// Due to the specification of vmrun, VmRun does not support this function.
    fn set_vm_by_id(&mut self, _id: &str) -> VmResult<()> {
        vmerr!(ErrorKind::UnsupportedCommand)
    }

    fn set_vm_by_name(&mut self, name: &str) -> VmResult<()> {
        if !self
            .list_vms()?
            .iter()
            .any(|vm| vm.name.as_deref() == Some(name))
        {
            return vmerr!(ErrorKind::VmNotFound);
        }
        Ok(())
    }

    fn set_vm_by_path(&mut self, path: &str) -> VmResult<()> {
        if !self
            .list_vms()?
            .iter()
            .any(|vm| vm.path.as_deref() == Some(path))
        {
            return vmerr!(ErrorKind::VmNotFound);
        }
        Ok(())
    }
}

impl PowerCmd for VmRun {
    fn start(&self) -> VmResult<()> {
        if self.is_running()? {
            return vmerr!(ErrorKind::InvalidPowerState(VmPowerState::Running));
        }
        self.start_vm(self.gui)
    }

    fn stop<D: Into<Option<Duration>>>(&self, _timeout: D) -> VmResult<()> {
        self.stop_vm(Some(false))
    }

    fn hard_stop(&self) -> VmResult<()> { self.stop_vm(Some(true)) }

    fn suspend(&self) -> VmResult<()> { self.suspend_vm(Some(true)) }

    fn resume(&self) -> VmResult<()> { self.start() }

    fn is_running(&self) -> VmResult<bool> {
        let vm_path = self.get_vm()?;
        Ok(self
            .list_running_vms()?
            .iter()
            .any(|vm| vm.path.as_deref().unwrap() == vm_path))
    }

    fn reboot<D: Into<Option<Duration>>>(&self, _timeout: D) -> VmResult<()> {
        self.reset_vm(Some(false))
    }

    fn hard_reboot(&self) -> VmResult<()> { self.reset_vm(Some(true)) }

    fn pause(&self) -> VmResult<()> { self.pause_vm() }

    fn unpause(&self) -> VmResult<()> { self.unpause_vm() }
}

impl SnapshotCmd for VmRun {
    fn list_snapshots(&self) -> VmResult<Vec<Snapshot>> {
        Self::list_snapshots(self)
    }

    fn take_snapshot(&self, name: &str) -> VmResult<()> { self.snapshot(name) }

    fn revert_snapshot(&self, name: &str) -> VmResult<()> {
        if !self.is_snapshot_exists(name)? {
            return vmerr!(ErrorKind::SnapshotNotFound);
        }
        self.revert_to_snapshot(name)
    }

    fn delete_snapshot(&self, name: &str) -> VmResult<()> {
        if !self.is_snapshot_exists(name)? {
            return vmerr!(ErrorKind::SnapshotNotFound);
        }
        self.delete_snapshot(name, true)
    }
}

impl GuestCmd for VmRun {
    fn exec_cmd(&self, guest_args: &[&str]) -> VmResult<()> {
        self.run_program_in_guest(true, true, false, guest_args)
    }

    fn copy_from_guest_to_host(
        &self,
        from_guest_path: &str,
        to_host_path: &str,
    ) -> VmResult<()> {
        self.copy_file_from_guest_to_host(from_guest_path, to_host_path)
    }

    fn copy_from_host_to_guest(
        &self,
        from_host_path: &str,
        to_guest_path: &str,
    ) -> VmResult<()> {
        self.copy_file_from_guest_to_host(from_host_path, to_guest_path)
    }
}
