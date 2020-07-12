use crate::types::*;
use std::process::Command;

/// Returns (stdout, stderr).
fn execute(cmd: &mut Command) -> VMResult<(String, String)> {
    match cmd.output() {
        Ok(o) => {
            let stdout = match String::from_utf8(o.stdout) {
                Ok(s) => s,
                Err(x) => return Err(VMError::from(Repr::Unknown(format!("Failed to convert stdout: {}", x.to_string())))),
            };
            let stderr = match String::from_utf8(o.stderr) {
                Ok(s) => s,
                Err(x) => return Err(VMError::from(Repr::Unknown(format!("Failed to convert stderr: {}", x.to_string())))),
            };
            Ok((stdout, stderr))
        }
        Err(x) => Err(VMError::from(ErrorKind::ExecutionFailed(x.to_string())))
    }
}

pub struct VBoxManage {
    path: String,
    vm: String,
    guest_username: Option<String>,
    guest_password: Option<String>,
    guest_password_file: Option<String>,
    guest_domain: Option<String>,
}

impl VBoxManage {
    pub fn new() -> Self {
        Self {
            path: "vboxmanage".to_string(),
            vm: "".to_string(),
            guest_username: None,
            guest_password: None,
            guest_password_file: None,
            guest_domain: None,
        }
    }

    pub fn executable_path<T: Into<String>>(mut self, path: T) -> Self {
        self.path = path.into().trim().to_string();
        self
    }

    pub fn vm<T: Into<String>>(mut self, vm: T) -> Self {
        self.vm = vm.into();
        self
    }

    pub fn guest_username<T: Into<String>>(mut self, guest_username: T) -> Self {
        self.guest_username = Some(guest_username.into());
        self
    }

    pub fn guest_password<T: Into<String>>(mut self, guest_password: T) -> Self {
        self.guest_password = Some(guest_password.into());
        self
    }

    pub fn guest_password_file<T: Into<String>>(mut self, guest_password_file: T) -> Self {
        self.guest_password_file = Some(guest_password_file.into());
        self
    }

    pub fn guest_domain<T: Into<String>>(mut self, guest_domain: T) -> Self {
        self.guest_domain = Some(guest_domain.into());
        self
    }

    fn build_auth(&self) -> Vec<&str> {
        let mut v = vec![];
        if let Some(x) = &self.guest_username { v.extend(&["--username", x]); }
        if let Some(x) = &self.guest_password { v.extend(&["--password", x]); }
        if let Some(x) = &self.guest_password_file { v.extend(&["--passwordfile", x]); }
        if let Some(x) = &self.guest_domain { v.extend(&["--domain", x]); }
        v
    }

    #[inline]
    fn handle_error(s: &str) -> VMResult<String> {
        if s.starts_with("Could not find a registered machine named ") {
            return Err(VMError::from(ErrorKind::VMNotFound));
        }
        if s.starts_with("Could not find a snapshot named ") {
            return Err(VMError::from(ErrorKind::SnapshotNotFound));
        }
        if s.starts_with("The specified user was not able to logon on guest") {
            return Err(VMError::from(ErrorKind::AuthenticationFailed));
        }
        if s.starts_with("FsObjQueryInfo failed on") || s.starts_with("File ") {
            let s = s.lines().last().unwrap();
            return Err(VMError::from(ErrorKind::FileError(s[s.rfind(":").unwrap() + 2..].to_string())));
        }
        if s.ends_with(" is not currently running") || s.find("is not running").is_some() {
            return Err(VMError::from(ErrorKind::VMIsNotPoweredOn));
        }
        if s.lines().next().unwrap().ends_with("is already locked by a session (or being locked or unlocked)") {
            return Err(VMError::from(ErrorKind::VMIsPoweredOn));
        }
        Err(VMError::from(Repr::Unknown(format!("Unknown error: {}", s))))
    }

    #[inline]
    fn check(s: String) -> VMResult<String> {
        const ERROR_STR: &str = "VBoxManage.exe: error: ";
        if s.starts_with(ERROR_STR) {
            Self::handle_error(&s[ERROR_STR.len()..].trim())
        } else {
            Ok(s)
        }
    }

    fn vbox_exec(cmd: &mut Command) -> VMResult<String> {
        let (stdout, stderr) = execute(cmd)?;
        if stderr.len() != 0 {
            Self::check(stderr)
        } else {
            Ok(stdout)
        }
    }

    /// Returns VMResult<()>.
    #[inline]
    fn vbox_exec2(cmd: &mut Command) -> VMResult<()> {
        Self::vbox_exec(cmd)?;
        Ok(())
    }

    #[inline]
    fn cmd(&self) -> Command { Command::new(&self.path) }

    pub fn version(&self) -> VMResult<String> {
        Ok(Self::vbox_exec(self.cmd().arg("-v"))?.trim().to_string())
    }

    pub fn list_vms(&self) -> VMResult<Vec<VM>> {
        let s = Self::vbox_exec(self.cmd().args(&["list", "vms"]))?;
        // "vm name" {uuid}
        Ok(s.lines()
            .map(|x| {
                let v = x.rsplitn(2, " ").collect::<Vec<&str>>();
                VM {
                    id: Some(v[0].to_string()),
                    name: Some(v[1][1..v[1].len() - 1].to_string()),
                    path: None,
                }
            }).collect())
    }

    pub fn start_vm(&self) -> VMResult<()> {
        Self::vbox_exec2(self.cmd().args(&["startvm", &self.vm]))
    }

    pub fn poweron(&self) -> VMResult<()> {
        Self::vbox_exec2(self.cmd().args(&["controlvm", &self.vm, "poweron"]))
    }

    pub fn poweroff(&self) -> VMResult<()> {
        Self::vbox_exec2(self.cmd().args(&["controlvm", &self.vm, "poweroff"]))
    }

    pub fn acpi_power_button(&self) -> VMResult<()> {
        Self::vbox_exec2(self.cmd().args(&["controlvm", &self.vm, "acpipowerbutton"]))
    }

    pub fn reset(&self) -> VMResult<()> {
        Self::vbox_exec2(self.cmd().args(&["controlvm", &self.vm, "reset"]))
    }

    pub fn pause(&self) -> VMResult<()> {
        Self::vbox_exec2(self.cmd().args(&["controlvm", &self.vm, "pause"]))
    }

    pub fn resume(&self) -> VMResult<()> {
        Self::vbox_exec2(self.cmd().args(&["controlvm", &self.vm, "resume"]))
    }

    pub fn save_state(&self) -> VMResult<()> {
        Self::vbox_exec2(self.cmd().args(&["controlvm", &self.vm, "savestate"]))
    }

    /// Returns (name, UUID).
    pub fn list_snapshots(&self) -> VMResult<Vec<Snapshot>> {
        let s = Self::vbox_exec(self.cmd().args(&["snapshot", &self.vm, "list"]))?;
        Ok(s.lines()
            .map(|x| {
                // "   Name: ss_name (UUID: ....)"
                let pos = x.rfind(" ").unwrap(); // UUID pos
                let name = &x.trim_start()[6..pos];
                let uuid = &x[pos + 2..x.len() - 1];
                Snapshot {
                    id: Some(uuid.to_string()),
                    name: Some(name.to_string()),
                    detail: None,
                }
            }).collect())
    }

    pub fn take_snapshot(&self, name: &str, description: Option<&str>, is_live: bool) -> VMResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["snapshot", &self.vm, "take", name]);
        if let Some(x) = description { cmd.args(&["--description", x]); }
        if is_live { cmd.arg("--live"); }
        Self::vbox_exec2(&mut cmd)
    }

    pub fn delete_snapshot(&self, name: &str) -> VMResult<()> {
        Self::vbox_exec2(self.cmd().args(&["snapshot", &self.vm, "delete", name]))
    }

    pub fn restore_snapshot(&self, name: &str) -> VMResult<()> {
        Self::vbox_exec2(self.cmd().args(&["snapshot", &self.vm, "restore", name]))
    }

    pub fn restore_current_snapshot(&self) -> VMResult<()> {
        Self::vbox_exec2(self.cmd().args(&["snapshot", &self.vm, "restorecurrent"]))
    }

    pub fn run(&self, guest_args: &[&str]) -> VMResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["guestcontrol", &self.vm, "run"]);
        cmd.args(self.build_auth());
        cmd.args(guest_args);
        Self::vbox_exec2(&mut cmd)
    }

    pub fn copy_from(&self, from_guest_path: &str, to_host_path: &str) -> VMResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["guestcontrol", &self.vm, "copyfrom"]);
        cmd.args(self.build_auth());
        cmd.args(&[from_guest_path, to_host_path]);
        Self::vbox_exec2(&mut cmd)
    }

    pub fn copy_to(&self, from_host_path: &str, to_guest_path: &str) -> VMResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["guestcontrol", &self.vm, "copyto"]);
        cmd.args(self.build_auth());
        cmd.args(&[from_host_path, to_guest_path]);
        Self::vbox_exec2(&mut cmd)
    }

    pub fn keyboard_put_scancode<T: Iterator<Item=u8>>(&self, v: T) -> VMResult<()> {
        use std::fmt::Write;
        let mut cmd = self.cmd();
        cmd.args(&["controlvm", &self.vm, "keyboardputscancode"]);
        cmd.args(self.build_auth());

        cmd.args(v.into_iter().map(|x| {
            let mut ret = String::new();
            ret.write_fmt(format_args!("{:x}", x)).unwrap();
            ret
        }).collect::<Vec<String>>());
        Self::vbox_exec2(&mut cmd)
    }

    pub fn keyboard_put_string(&self, v: &[&str]) -> VMResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["controlvm", &self.vm, "keyboardputstring"]);
        cmd.args(self.build_auth());
        cmd.args(v);
        Self::vbox_exec2(&mut cmd)
    }
}

impl PowerCmd for VBoxManage {
    fn start(&self) -> VMResult<()> { self.poweron() }

    fn stop(&self) -> VMResult<()> { self.acpi_power_button() }

    fn hard_stop(&self) -> VMResult<()> { self.poweroff() }

    fn suspend(&self) -> VMResult<()> { self.save_state() }

    fn resume(&self) -> VMResult<()> { Self::resume(self) }

    fn is_running(&self) -> VMResult<bool> { Ok(self.start_vm() != Ok(())) }

    fn reboot(&self) -> VMResult<()> { self.reset() }

    fn hard_reboot(&self) -> VMResult<()> {
        Err(VMError::from(ErrorKind::UnsupportedCommand))
    }

    fn pause(&self) -> VMResult<()> { VBoxManage::pause(self) }

    fn unpause(&self) -> VMResult<()> { self.resume() }
}

impl GuestCmd for VBoxManage {
    fn run_command(&self, guest_args: &[&str]) -> VMResult<()> {
        self.run(guest_args)
    }

    fn copy_from_guest_to_host(&mut self, from_guest_path: &str, to_host_path: &str) -> VMResult<()> {
        self.copy_from(from_guest_path, to_host_path)
    }

    fn copy_from_host_to_guest(&mut self, from_host_path: &str, to_guest_path: &str) -> VMResult<()> {
        self.copy_to(from_host_path, to_guest_path)
    }
}

impl SnapshotCmd for VBoxManage {
    fn list_snapshots(&mut self) -> VMResult<Vec<Snapshot>> {
        Self::list_snapshots(self)
    }

    fn take_snapshot(&mut self, name: &str) -> VMResult<()> {
        Self::take_snapshot(self, name, None, true)
    }

    fn revert_snapshot(&mut self, name: &str) -> VMResult<()> {
        self.restore_snapshot(name)
    }

    fn delete_snapshot(&mut self, name: &str) -> VMResult<()> {
        Self::delete_snapshot(self, name)
    }
}

