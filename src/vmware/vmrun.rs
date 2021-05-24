use crate::{
    types::*,
    vmware::{read_vmware_inventory, read_vmware_preferences},
};
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

pub enum WriteVar<'a> {
    GuestVar(&'a str, &'a str),
    RuntimeConfig(&'a str, &'a str),
    GuestEnv(&'a str, &'a str),
}

pub enum ReadVar<'a> {
    GuestVar(&'a str),
    RuntimeConfig(&'a str),
    GuestEnv(&'a str),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProcInfo {
    pub pid: u32,
    pub owner: String,
    pub cmd: String,
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
        starts_err!(
            s,
            "The VMware Tools are not running in the virtual machine: ",
            ServiceIsNotRunning
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
        let vms = if self.executable_path.contains("VMware Player") {
            // Player
            read_vmware_preferences(&format!(r"{}\VMware\preferences.ini", p))?
        } else {
            // Workstation
            read_vmware_inventory(&format!(r"{}\VMware\inventory.vmls", p))?
        };

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

    pub fn file_exists_in_guest(&self, guest_path: &str) -> VmResult<bool> {
        let s = Self::exec(self.cmd().args(&[
            "fileExistsInGuest",
            self.get_vm()?,
            guest_path,
        ]))?;
        match s.as_str().trim() {
            "The file exists." => Ok(true),
            "The file does not exist." => Ok(false),
            _ => vmerr!(ErrorKind::UnexpectedResponse(s)),
        }
    }

    pub fn directory_exists_in_guest(
        &self,
        guest_path: &str,
    ) -> VmResult<bool> {
        let s = Self::exec(self.cmd().args(&[
            "directoryExistsInGuest",
            self.get_vm()?,
            guest_path,
        ]))?;
        match s.as_str().trim() {
            "The directory exists." => Ok(true),
            "The directory does not exist." => Ok(false),
            _ => vmerr!(ErrorKind::UnexpectedResponse(s)),
        }
    }

    pub fn set_shared_folder_state(
        &self,
        name: &str,
        host_path: &str,
        writable: bool,
    ) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["setSharedFolderState", name, host_path]);
        cmd.arg(if writable { "writable" } else { "readonly" });
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn add_shared_folder(
        &self,
        name: &str,
        host_path: &str,
    ) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["addSharedFolder", name, host_path]);
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn remove_shared_folder(&self, name: &str) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["removeSharedFolder", name]);
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn enable_shared_folders(
        &self,
        name: &str,
        only_runtime: bool,
    ) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["enableSharedFolders", name]);
        if only_runtime {
            cmd.arg("runtime");
        }
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn disable_shared_folders(
        &self,
        name: &str,
        only_runtime: bool,
    ) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["disableSharedFolders", name]);
        if only_runtime {
            cmd.arg("runtime");
        }
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn list_processes_in_guest(&self) -> VmResult<Vec<ProcInfo>> {
        let s = Self::exec(
            self.cmd().args(&["listProcessesInGuest", self.get_vm()?]),
        )?;
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
        for l in l {
            let v: Vec<&str> = l.splitn(3, ", ").collect();
            assert_eq!(v.len(), 3);
            let (pid, owner, cmd) = (v[0], v[1], v[2]);
            let pid = pid.strip_prefix("pid=").expect("Unexpected pid");
            let pid: u32 = pid.parse().unwrap();
            let owner = owner
                .strip_prefix("owner=")
                .expect("Unexpected owner")
                .to_string();
            let cmd = cmd
                .strip_prefix("cmd=")
                .expect("Unexpected process command")
                .to_string();
            ret.push(ProcInfo { pid, owner, cmd })
        }
        Ok(ret)
    }

    pub fn kill_process_in_guest(&self, pid: u32) -> VmResult<()> {
        Self::exec(self.cmd().args(&[
            "killProcessInGuest",
            self.get_vm()?,
            &pid.to_string(),
        ]))?;
        Ok(())
    }

    pub fn delete_file_in_guest(&self, guest_path: &str) -> VmResult<()> {
        Self::exec(self.cmd().args(&[
            "deleteFileInGuest",
            self.get_vm()?,
            guest_path,
        ]))?;
        Ok(())
    }

    pub fn create_directory_in_guest(&self, guest_path: &str) -> VmResult<()> {
        Self::exec(self.cmd().args(&[
            "createDirectoryInGuest",
            self.get_vm()?,
            guest_path,
        ]))?;
        Ok(())
    }

    pub fn delete_directory_in_guest(&self, guest_path: &str) -> VmResult<()> {
        Self::exec(self.cmd().args(&[
            "deleteDirectoryInGuest",
            self.get_vm()?,
            guest_path,
        ]))?;
        Ok(())
    }

    /// Creates a temp file in guest.
    ///
    /// Returns the path to the temp file.
    pub fn create_temp_file_in_guest(&self) -> VmResult<String> {
        let s = Self::exec(
            self.cmd().args(&["createTempFileInGuest", self.get_vm()?]),
        )?;
        Ok(s)
    }

    pub fn list_directory_in_guest(
        &self,
        guest_path: &str,
    ) -> VmResult<Vec<String>> {
        let s = Self::exec(self.cmd().args(&[
            "listDirectoryInGuest",
            self.get_vm()?,
            guest_path,
        ]))?;
        Ok(s.lines().skip(1).map(|x| x.to_string()).collect())
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

    pub fn rename_file_in_guest(
        &self,
        old_path: &str,
        new_path: &str,
    ) -> VmResult<()> {
        Self::exec(self.cmd().args(&[
            "renameFileInGuest",
            self.get_vm()?,
            old_path,
            new_path,
        ]))?;
        Ok(())
    }

    pub fn type_keystrokes_in_guest(&self, keystroke: &str) -> VmResult<()> {
        Self::exec(self.cmd().args(&[
            "typeKeystrokesInGuest",
            self.get_vm()?,
            keystroke,
        ]))?;
        Ok(())
    }

    pub fn capture_screen(&self, host_path: &str) -> VmResult<()> {
        Self::exec(self.cmd().args(&[
            "captureScreen",
            self.get_vm()?,
            host_path,
        ]))?;
        Ok(())
    }

    pub fn write_variable(&self, variable: WriteVar) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["writeVariable", self.get_vm()?]);
        match variable {
            WriteVar::GuestVar(name, value) => {
                cmd.args(&["guestVar", name, value])
            }
            WriteVar::RuntimeConfig(name, value) => {
                cmd.args(&["runtimeConfig", name, value])
            }
            WriteVar::GuestEnv(name, value) => {
                cmd.args(&["guestEnv", name, value])
            }
        };
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn read_variable(&self, variable: ReadVar) -> VmResult<Option<String>> {
        let mut cmd = self.cmd();
        cmd.args(&["readVariable", self.get_vm()?]);
        match variable {
            ReadVar::GuestVar(name) => cmd.args(&["guestVar", name]),
            ReadVar::RuntimeConfig(name) => cmd.args(&["runtimeConfig", name]),
            ReadVar::GuestEnv(name) => cmd.args(&["guestEnv", name]),
        };
        let s = Self::exec(&mut cmd)?;
        Ok(if s.is_empty() { None } else { Some(s) })
    }

    pub fn get_guest_ip_address(&self, wait: bool) -> VmResult<String> {
        let mut cmd = self.cmd();
        cmd.args(&["getGuestIPAddress", self.get_vm()?]);
        if wait {
            cmd.arg("-wait");
        }
        let s = Self::exec(&mut cmd)?;
        Ok(s)
    }

    pub fn install_tools(&self) -> VmResult<()> {
        Self::exec(self.cmd().args(&["installTools", self.get_vm()?]))?;
        Ok(())
    }

    pub fn check_tools_state(&self) -> VmResult<bool> {
        let s =
            Self::exec(self.cmd().args(&["checkToolsState", self.get_vm()?]))?;
        match s.as_str() {
            "installed" => Ok(true),
            "unknown" => Ok(false),
            "running" => Ok(true),
            _ => vmerr!(ErrorKind::UnexpectedResponse(s)),
        }
    }

    pub fn delete_vm(&self) -> VmResult<()> {
        Self::exec(self.cmd().args(&["deleteVM", self.get_vm()?]))?;
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
        for vm in self.list_vms()? {
            if vm.name.as_deref() == Some(name) {
                self.vm_path = vm.path;
                return Ok(());
            }
        }
        vmerr!(ErrorKind::VmNotFound)
    }

    fn set_vm_by_path(&mut self, path: &str) -> VmResult<()> {
        for vm in self.list_vms()? {
            if vm.path.as_deref() == Some(path) {
                self.vm_path = vm.path;
                return Ok(());
            }
        }
        vmerr!(ErrorKind::VmNotFound)
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
        self.copy_file_from_host_to_guest(from_host_path, to_guest_path)
    }
}
