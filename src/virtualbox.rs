use crate::types::*;
use crate::types::ErrorKind::UnexpectedResponse;
use encoding_rs::Encoding;
use std::process::Command;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct VBoxManage {
    path: String,
    vm: String,
    guest_username: Option<String>,
    guest_password: Option<String>,
    guest_password_file: Option<String>,
    guest_domain: Option<String>,
    encoding: &'static Encoding,
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
            encoding: encoding_rs::UTF_8,
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

    pub fn encoding(mut self, encoding_name: &str) -> Self {
        self.encoding = Encoding::for_label(encoding_name.as_bytes()).expect("Invalid encoding");
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
    fn handle_error(s: &str) -> VMError {
        starts_err!(s, "Could not find a registered machine named", ErrorKind::VMNotFound);
        starts_err!(s, "Could not find a snapshot named ", ErrorKind::SnapshotNotFound);
        starts_err!(s, "The specified user was not able to logon on guest", ErrorKind::GuestAuthenticationFailed);
        if s.starts_with("FsObjQueryInfo failed on") || s.starts_with("File ") {
            let s = s.lines().last().unwrap();
            return VMError::from(ErrorKind::FileError(s[s.rfind(":").unwrap() + 2..].to_string()));
        }
        if s.starts_with("Invalid machine state: PoweredOff") || s.starts_with("Machine in invalid state 1 -- powered off") {
            return VMError::from(ErrorKind::VMIsNotRunning);
        }
        if s.ends_with(" is not currently running") || s.find("is not running").is_some() {
            return VMError::from(ErrorKind::VMIsNotRunning);
        }
        if s.lines().next().unwrap().ends_with("is already locked by a session (or being locked or unlocked)") {
            return VMError::from(ErrorKind::VMIsRunning);
        }
        VMError::from(Repr::Unknown(format!("Unknown error: {}", s)))
    }

    #[inline]
    fn check(s: String) -> VMResult<String> {
        const ERROR_STR: &str = "VBoxManage.exe: error: ";
        if s.starts_with(ERROR_STR) {
            Err(Self::handle_error(&s[ERROR_STR.len()..].trim()))
        } else {
            Ok(s)
        }
    }

    fn vbox_exec(&self, cmd: &mut Command) -> VMResult<String> {
        let (stdout, stderr) = exec_cmd(self.encoding, cmd)?;
        if stderr.len() != 0 {
            Self::check(stderr)
        } else {
            Ok(stdout)
        }
    }

    #[inline]
    fn vbox_exec2(&self, cmd: &mut Command) -> VMResult<()> {
        self.vbox_exec(cmd)?;
        Ok(())
    }

    #[inline]
    fn cmd(&self) -> Command { Command::new(&self.path) }

    pub fn version(&self) -> VMResult<String> {
        Ok(self.vbox_exec(self.cmd().arg("-v"))?.trim().to_string())
    }

    pub fn list_vms(&self) -> VMResult<Vec<VM>> {
        let s = self.vbox_exec(self.cmd().args(&["list", "vms"]))?;
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

    pub fn show_vm_info(&self) -> VMResult<String> {
        self.vbox_exec(self.cmd().args(&["showvminfo", &self.vm, "--machinereadable"]))
    }

    pub fn start_vm(&self) -> VMResult<()> {
        self.vbox_exec2(self.cmd().args(&["startvm", &self.vm]))
    }

    pub fn poweroff_vm(&self) -> VMResult<()> {
        self.vbox_exec2(self.cmd().args(&["controlvm", &self.vm, "poweroff"]))
    }

    /// Sends ACPI shutdown signal.
    ///
    /// If the VM is running, this function returns Ok(()) regardless of whether the VM was shut down.
    pub fn acpi_power_button_vm(&self) -> VMResult<()> {
        self.vbox_exec2(self.cmd().args(&["controlvm", &self.vm, "acpipowerbutton"]))
    }

    pub fn reset_vm(&self) -> VMResult<()> {
        self.vbox_exec2(self.cmd().args(&["controlvm", &self.vm, "reset"]))
    }

    pub fn pause_vm(&self) -> VMResult<()> {
        self.vbox_exec2(self.cmd().args(&["controlvm", &self.vm, "pause"]))
    }

    pub fn resume_vm(&self) -> VMResult<()> {
        self.vbox_exec2(self.cmd().args(&["controlvm", &self.vm, "resume"]))
    }

    pub fn save_state_vm(&self) -> VMResult<()> {
        self.vbox_exec2(self.cmd().args(&["controlvm", &self.vm, "savestate"]))
    }

    pub fn list_snapshots(&self) -> VMResult<Vec<Snapshot>> {
        const SN_NAME: &str = "SnapshotName";
        const SN_UUID: &str = "SnapshotUUID";
        const SN_DESC: &str = "SnapshotDescription";
        #[derive(Eq, PartialEq)]
        enum State {
            Init,
            Name,
            UUID,
            Desc,
            DescCont,
        }
        let s = self.vbox_exec(self.cmd().args(&["snapshot", &self.vm, "list", "--machinereadable"]))?;
        let mut last_state = State::Init;

        let mut ret = vec![];
        let mut sn = Snapshot {
            id: None,
            name: None,
            detail: None,
        };
        let mut cur_detail = "".to_string();
        for x in s.lines() {
            let now_data = if x.starts_with(SN_NAME) {
                State::Name
            } else if x.starts_with(SN_UUID) {
                State::UUID
            } else if x.starts_with(SN_DESC) {
                State::Desc
            } else if x.starts_with("CurrentSnapshotName=\"") {
                // End
                return if last_state == State::Desc || last_state == State::DescCont {
                    cur_detail.pop(); // Remove last "
                    Ok(ret)
                } else { vmerr!(ErrorKind::UnexpectedResponse(x.to_string())) };
            } else {
                State::DescCont
            };
            match last_state {
                State::Init => {
                    match now_data {
                        State::Name => {
                            let p = x.find("=").expect("Invalid name");
                            sn.name = Some(x[p + 2..x.len() - 1].to_string());
                            last_state = State::Name;
                        }
                        _ => return vmerr!(ErrorKind::UnexpectedResponse(x.to_string())),
                    }
                }
                State::Name => {
                    match now_data {
                        State::UUID => {
                            let p = x.find("=").expect("Invalid UUID");
                            sn.id = Some(x[p + 2..x.len() - 1].to_string());
                            last_state = State::UUID;
                        }
                        _ => return vmerr!(ErrorKind::UnexpectedResponse(x.to_string())),
                    }
                }
                State::UUID => {
                    match now_data {
                        State::Desc => {
                            let p = x.find("=").expect("Invalid description");
                            cur_detail = x[p + 2..].to_string();
                            last_state = State::Desc;
                        }
                        _ => return vmerr!(ErrorKind::UnexpectedResponse(x.to_string())),
                    }
                }
                State::Desc => {
                    match now_data {
                        State::Name => {
                            sn.detail = Some(cur_detail[..cur_detail.len() - 1].to_string());
                            ret.push(sn.clone());
                            cur_detail = "".to_string();
                            let p = x.find("=").expect("Invalid name");
                            sn.name = Some(x[p + 2..x.len() - 1].to_string());
                            last_state = State::Name;
                        }
                        State::DescCont => {
                            #[cfg(target_os = "windows")]
                                { cur_detail += "\r\n"; }
                            #[cfg(not(target_os = "windows"))]
                                { cur_detail += "\n"; }
                            cur_detail += x;
                            last_state = State::DescCont;
                        }
                        _ => return vmerr!(ErrorKind::UnexpectedResponse(x.to_string())),
                    }
                }
                State::DescCont => {
                    match now_data {
                        State::Name => {
                            sn.detail = Some(cur_detail[..cur_detail.len() - 1].to_string());
                            ret.push(sn.clone());
                            cur_detail = "".to_string();
                            let p = x.find("=").expect("Invalid name");
                            sn.name = Some(x[p + 2..x.len() - 1].to_string());
                            last_state = State::Name;
                        }
                        State::DescCont => {
                            #[cfg(target_os = "windows")]
                                { cur_detail += "\r\n"; }
                            #[cfg(not(target_os = "windows"))]
                                { cur_detail += "\n"; }
                            cur_detail += x;
                            last_state = State::DescCont;
                        }
                        _ => return vmerr!(ErrorKind::UnexpectedResponse(x.to_string())),
                    }
                }
            };
        }
        Ok(ret)
    }

    pub fn take_snapshot(&self, name: &str, description: Option<&str>, is_live: bool) -> VMResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["snapshot", &self.vm, "take", name]);
        if let Some(x) = description { cmd.args(&["--description", x]); }
        if is_live { cmd.arg("--live"); }
        self.vbox_exec2(&mut cmd)
    }

    pub fn delete_snapshot(&self, name: &str) -> VMResult<()> {
        self.vbox_exec2(self.cmd().args(&["snapshot", &self.vm, "delete", name]))
    }

    pub fn restore_snapshot(&self, name: &str) -> VMResult<()> {
        self.vbox_exec2(self.cmd().args(&["snapshot", &self.vm, "restore", name]))
    }

    pub fn restore_current_snapshot(&self) -> VMResult<()> {
        self.vbox_exec2(self.cmd().args(&["snapshot", &self.vm, "restorecurrent"]))
    }

    pub fn run(&self, guest_args: &[&str]) -> VMResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["guestcontrol", &self.vm, "run"]);
        cmd.args(self.build_auth());
        cmd.args(guest_args);
        self.vbox_exec2(&mut cmd)
    }

    pub fn copy_from(&self, from_guest_path: &str, to_host_path: &str) -> VMResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["guestcontrol", &self.vm, "copyfrom"]);
        cmd.args(self.build_auth());
        cmd.args(&[from_guest_path, to_host_path]);
        self.vbox_exec2(&mut cmd)
    }

    pub fn copy_to(&self, from_host_path: &str, to_guest_path: &str) -> VMResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["guestcontrol", &self.vm, "copyto"]);
        cmd.args(self.build_auth());
        cmd.args(&[from_host_path, to_guest_path]);
        self.vbox_exec2(&mut cmd)
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
        self.vbox_exec2(&mut cmd)
    }

    pub fn keyboard_put_string(&self, v: &[&str]) -> VMResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["controlvm", &self.vm, "keyboardputstring"]);
        cmd.args(self.build_auth());
        cmd.args(v);
        self.vbox_exec2(&mut cmd)
    }
}

impl PowerCmd for VBoxManage {
    fn start(&self) -> VMResult<()> { self.start_vm() }

    /// Sends ACPI shutdown signals until the VM to stop.
    fn stop(&self) -> VMResult<()> {
        loop {
            // Polling every second.
            let status = self.acpi_power_button_vm();
            if status == vmerr!(ErrorKind::VMIsNotRunning) {
                return Ok(());
            } else if let Err(x) = status {
                return Err(x);
            }
            std::thread::sleep(Duration::from_secs(1))
        }
    }

    fn hard_stop(&self) -> VMResult<()> {
        self.poweroff_vm()
    }

    fn suspend(&self) -> VMResult<()> {
        loop {
            let status = self.save_state_vm();
            if status == vmerr!(ErrorKind::VMIsNotRunning) {
                return Ok(());
            } else if status == vmerr!(Repr::Unknown("Machine in invalid state 2 -- saved".to_string())) {
                // Do nothing
            } else if let Err(x) = status {
                return Err(x);
            }
            std::thread::sleep(Duration::from_secs(1))
        }
    }

    fn resume(&self) -> VMResult<()> { self.start_vm() }

    fn is_running(&self) -> VMResult<bool> {
        const VMS: &str = "VMState=\"";
        let s = self.show_vm_info()?;
        for x in s.lines() {
            if x.starts_with(VMS) {
                return Ok(&x[VMS.len()..x.len() - 1] == "running");
            }
        }
        vmerr!(UnexpectedResponse(s))
    }

    fn reboot(&self) -> VMResult<()> {
        self.stop()?;
        loop {
            let status = self.start();
            if status == Ok(()) { return Ok(()); } else if status == vmerr!(ErrorKind::VMIsRunning) {
                // Do nothing
            } else if let Err(x) = status { return Err(x); }
        }
    }

    fn hard_reboot(&self) -> VMResult<()> { self.reset_vm() }

    fn pause(&self) -> VMResult<()> { VBoxManage::pause_vm(self) }

    fn unpause(&self) -> VMResult<()> { self.resume_vm() }
}

impl GuestCmd for VBoxManage {
    fn run_command(&self, guest_args: &[&str]) -> VMResult<()> {
        self.run(guest_args)
    }

    fn copy_from_guest_to_host(&self, from_guest_path: &str, to_host_path: &str) -> VMResult<()> {
        self.copy_from(from_guest_path, to_host_path)
    }

    fn copy_from_host_to_guest(&self, from_host_path: &str, to_guest_path: &str) -> VMResult<()> {
        self.copy_to(from_host_path, to_guest_path)
    }
}

impl SnapshotCmd for VBoxManage {
    fn list_snapshots(&self) -> VMResult<Vec<Snapshot>> {
        Self::list_snapshots(self)
    }

    fn take_snapshot(&self, name: &str) -> VMResult<()> {
        Self::take_snapshot(self, name, None, true)
    }

    fn revert_snapshot(&self, name: &str) -> VMResult<()> {
        self.restore_snapshot(name)
    }

    fn delete_snapshot(&self, name: &str) -> VMResult<()> {
        Self::delete_snapshot(self, name)
    }
}

