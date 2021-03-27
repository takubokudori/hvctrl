// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
//! VBoxManage controller.
use crate::types::*;
use std::process::Command;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct VBoxManage {
    executable_path: String,
    vm: String,
    guest_username: Option<String>,
    guest_password: Option<String>,
    guest_password_file: Option<String>,
    guest_domain: Option<String>,
}

impl Default for VBoxManage {
    fn default() -> Self {
        Self::new()
    }
}

impl VBoxManage {
    pub fn new() -> Self {
        Self {
            executable_path: "vboxmanage".to_string(),
            vm: "".to_string(),
            guest_username: None,
            guest_password: None,
            guest_password_file: None,
            guest_domain: None,
        }
    }

    /// Sets the path to VBoxManage.
    pub fn executable_path<T: Into<String>>(mut self, path: T) -> Self {
        self.executable_path = path.into().trim().to_string();
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
        if let Some(x) = &self.guest_username {
            v.extend(&["--username", x]);
        }
        if let Some(x) = &self.guest_password {
            v.extend(&["--password", x]);
        }
        if let Some(x) = &self.guest_password_file {
            v.extend(&["--passwordfile", x]);
        }
        if let Some(x) = &self.guest_domain {
            v.extend(&["--domain", x]);
        }
        v
    }

    #[inline]
    fn handle_error(s: &str) -> VmError {
        starts_err!(
            s,
            "Could not find a registered machine named",
            ErrorKind::VmNotFound
        );
        starts_err!(
            s,
            "Could not find a snapshot named ",
            ErrorKind::SnapshotNotFound
        );
        starts_err!(
            s,
            "The specified user was not able to logon on guest",
            ErrorKind::GuestAuthenticationFailed
        );
        if s.starts_with("FsObjQueryInfo failed on") || s.starts_with("File ") {
            let s = s.lines().last().unwrap();
            return VmError::from(ErrorKind::FileError(
                s[s.rfind(':').unwrap() + 2..].to_string(),
            ));
        }
        if s.starts_with("Invalid machine state: PoweredOff")
            || s.starts_with("Machine in invalid state 1 -- powered off")
        {
            return VmError::from(ErrorKind::VmIsNotRunning);
        }
        if s.ends_with(" is not currently running") || s.contains("is not running") {
            return VmError::from(ErrorKind::VmIsNotRunning);
        }
        if s.lines()
            .next()
            .unwrap()
            .ends_with("is already locked by a session (or being locked or unlocked)")
        {
            return VmError::from(ErrorKind::VmIsRunning);
        }
        VmError::from(Repr::Unknown(format!("Unknown error: {}", s)))
    }

    #[inline]
    fn check(s: String) -> VmResult<String> {
        const ERROR_STR: &str = "vboxmanage.exe: error: ";
        if (&s[..ERROR_STR.len()])
            .to_ascii_lowercase()
            .starts_with(ERROR_STR)
        {
            Err(Self::handle_error(&s[ERROR_STR.len()..].trim()))
        } else {
            Ok(s)
        }
    }

    fn exec(&self, cmd: &mut Command) -> VmResult<String> {
        let (stdout, stderr) = exec_cmd(cmd)?;
        if !stderr.is_empty() {
            Self::check(stderr)
        } else {
            Ok(stdout)
        }
    }

    #[inline]
    fn exec2(&self, cmd: &mut Command) -> VmResult<()> {
        self.exec(cmd)?;
        Ok(())
    }

    #[inline]
    fn cmd(&self) -> Command {
        Command::new(&self.executable_path)
    }

    pub fn version(&self) -> VmResult<String> {
        Ok(self.exec(self.cmd().arg("-v"))?.trim().to_string())
    }

    pub fn list_vms(&self) -> VmResult<Vec<Vm>> {
        let s = self.exec(self.cmd().args(&["list", "vms"]))?;
        // "vm name" {uuid}
        Ok(s.lines()
            .map(|x| {
                let v = x.rsplitn(2, ' ').collect::<Vec<&str>>();
                Vm {
                    id: Some(v[0].to_string()),
                    name: Some(v[1][1..v[1].len() - 1].to_string()),
                    path: None,
                }
            })
            .collect())
    }

    pub fn show_vm_info(&self) -> VmResult<String> {
        self.exec(
            self.cmd()
                .args(&["showvminfo", &self.vm, "--machinereadable"]),
        )
    }

    pub fn start_vm(&self) -> VmResult<()> {
        self.exec2(self.cmd().args(&["startvm", &self.vm]))
    }

    pub fn poweroff_vm(&self) -> VmResult<()> {
        self.exec2(self.cmd().args(&["controlvm", &self.vm, "poweroff"]))
    }

    /// Sends ACPI shutdown signal.
    ///
    /// If the VM is running, this function returns Ok(()) regardless of whether the VM was shut down.
    pub fn acpi_power_button_vm(&self) -> VmResult<()> {
        self.exec2(self.cmd().args(&["controlvm", &self.vm, "acpipowerbutton"]))
    }

    pub fn reset_vm(&self) -> VmResult<()> {
        self.exec2(self.cmd().args(&["controlvm", &self.vm, "reset"]))
    }

    pub fn pause_vm(&self) -> VmResult<()> {
        self.exec2(self.cmd().args(&["controlvm", &self.vm, "pause"]))
    }

    pub fn resume_vm(&self) -> VmResult<()> {
        self.exec2(self.cmd().args(&["controlvm", &self.vm, "resume"]))
    }

    pub fn save_state_vm(&self) -> VmResult<()> {
        self.exec2(self.cmd().args(&["controlvm", &self.vm, "savestate"]))
    }

    pub fn list_snapshots(&self) -> VmResult<Vec<Snapshot>> {
        const SN_NAME: &str = "SnapshotName";
        const SN_UUID: &str = "SnapshotUUID";
        const SN_DESC: &str = "SnapshotDescription";
        #[derive(Eq, PartialEq)]
        enum State {
            Init,
            Name,
            Uuid,
            Desc,
            DescCont,
        }
        let s = self.exec(
            self.cmd()
                .args(&["snapshot", &self.vm, "list", "--machinereadable"]),
        )?;
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
                State::Uuid
            } else if x.starts_with(SN_DESC) {
                State::Desc
            } else if x.starts_with("CurrentSnapshotName=\"") {
                // End
                return if last_state == State::Desc || last_state == State::DescCont {
                    cur_detail.pop(); // Remove last "
                    Ok(ret)
                } else {
                    vmerr!(ErrorKind::UnexpectedResponse(x.to_string()))
                };
            } else {
                State::DescCont
            };
            match last_state {
                State::Init => match now_data {
                    State::Name => {
                        let p = x.find('=').expect("Invalid name");
                        sn.name = Some(x[p + 2..x.len() - 1].to_string());
                        last_state = State::Name;
                    }
                    _ => return vmerr!(ErrorKind::UnexpectedResponse(x.to_string())),
                },
                State::Name => match now_data {
                    State::Uuid => {
                        let p = x.find('=').expect("Invalid UUID");
                        sn.id = Some(x[p + 2..x.len() - 1].to_string());
                        last_state = State::Uuid;
                    }
                    _ => return vmerr!(ErrorKind::UnexpectedResponse(x.to_string())),
                },
                State::Uuid => match now_data {
                    State::Desc => {
                        let p = x.find('=').expect("Invalid description");
                        cur_detail = x[p + 2..].to_string();
                        last_state = State::Desc;
                    }
                    _ => return vmerr!(ErrorKind::UnexpectedResponse(x.to_string())),
                },
                State::Desc => match now_data {
                    State::Name => {
                        sn.detail = Some(cur_detail[..cur_detail.len() - 1].to_string());
                        ret.push(sn.clone());
                        cur_detail = "".to_string();
                        let p = x.find('=').expect("Invalid name");
                        sn.name = Some(x[p + 2..x.len() - 1].to_string());
                        last_state = State::Name;
                    }
                    State::DescCont => {
                        #[cfg(target_os = "windows")]
                            {
                                cur_detail += "\r\n";
                            }
                        #[cfg(not(target_os = "windows"))]
                            {
                                cur_detail += "\n";
                            }
                        cur_detail += x;
                        last_state = State::DescCont;
                    }
                    _ => return vmerr!(ErrorKind::UnexpectedResponse(x.to_string())),
                },
                State::DescCont => match now_data {
                    State::Name => {
                        sn.detail = Some(cur_detail[..cur_detail.len() - 1].to_string());
                        ret.push(sn.clone());
                        cur_detail = "".to_string();
                        let p = x.find('=').expect("Invalid name");
                        sn.name = Some(x[p + 2..x.len() - 1].to_string());
                        last_state = State::Name;
                    }
                    State::DescCont => {
                        #[cfg(target_os = "windows")]
                            {
                                cur_detail += "\r\n";
                            }
                        #[cfg(not(target_os = "windows"))]
                            {
                                cur_detail += "\n";
                            }
                        cur_detail += x;
                        last_state = State::DescCont;
                    }
                    _ => return vmerr!(ErrorKind::UnexpectedResponse(x.to_string())),
                },
            };
        }
        Ok(ret)
    }

    pub fn take_snapshot(
        &self,
        name: &str,
        description: Option<&str>,
        is_live: bool,
    ) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["snapshot", &self.vm, "take", name]);
        if let Some(x) = description {
            cmd.args(&["--description", x]);
        }
        if is_live {
            cmd.arg("--live");
        }
        self.exec2(&mut cmd)
    }

    pub fn delete_snapshot(&self, name: &str) -> VmResult<()> {
        self.exec2(self.cmd().args(&["snapshot", &self.vm, "delete", name]))
    }

    pub fn restore_snapshot(&self, name: &str) -> VmResult<()> {
        self.exec2(self.cmd().args(&["snapshot", &self.vm, "restore", name]))
    }

    pub fn restore_current_snapshot(&self) -> VmResult<()> {
        self.exec2(self.cmd().args(&["snapshot", &self.vm, "restorecurrent"]))
    }

    pub fn run(&self, guest_args: &[&str]) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["guestcontrol", &self.vm, "run"]);
        cmd.args(self.build_auth());
        cmd.args(guest_args);
        self.exec2(&mut cmd)
    }

    pub fn copy_from(&self, from_guest_path: &str, to_host_path: &str) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["guestcontrol", &self.vm, "copyfrom"]);
        cmd.args(self.build_auth());
        cmd.args(&[from_guest_path, to_host_path]);
        self.exec2(&mut cmd)
    }

    pub fn copy_to(&self, from_host_path: &str, to_guest_path: &str) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["guestcontrol", &self.vm, "copyto"]);
        cmd.args(self.build_auth());
        cmd.args(&[from_host_path, to_guest_path]);
        self.exec2(&mut cmd)
    }

    pub fn keyboard_put_scancode<T: Iterator<Item=u8>>(&self, v: T) -> VmResult<()> {
        use std::fmt::Write;
        let mut cmd = self.cmd();
        cmd.args(&["controlvm", &self.vm, "keyboardputscancode"]);
        cmd.args(self.build_auth());

        cmd.args(
            v.into_iter()
                .map(|x| {
                    let mut ret = String::new();
                    ret.write_fmt(format_args!("{:x}", x)).unwrap();
                    ret
                })
                .collect::<Vec<String>>(),
        );
        self.exec2(&mut cmd)
    }

    pub fn keyboard_put_string(&self, v: &[&str]) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["controlvm", &self.vm, "keyboardputstring"]);
        cmd.args(self.build_auth());
        cmd.args(v);
        self.exec2(&mut cmd)
    }
}

impl PowerCmd for VBoxManage {
    fn start(&self) -> VmResult<()> {
        self.start_vm()
    }

    /// Sends ACPI shutdown signals until the VM to stop.
    fn stop(&self) -> VmResult<()> {
        loop {
            // Polling every second.
            let status = self.acpi_power_button_vm();
            if status == vmerr!(ErrorKind::VmIsNotRunning) {
                return Ok(());
            } else if let Err(x) = status {
                return Err(x);
            }
            std::thread::sleep(Duration::from_secs(1))
        }
    }

    fn hard_stop(&self) -> VmResult<()> {
        self.poweroff_vm()
    }

    fn suspend(&self) -> VmResult<()> {
        loop {
            let status = self.save_state_vm();
            if status == vmerr!(ErrorKind::VmIsNotRunning) {
                return Ok(());
            } else if status
                == vmerr!(Repr::Unknown(
                    "Machine in invalid state 2 -- saved".to_string()
                ))
            {
                // Do nothing
            } else if let Err(x) = status {
                return Err(x);
            }
            std::thread::sleep(Duration::from_secs(1))
        }
    }

    fn resume(&self) -> VmResult<()> {
        self.start_vm()
    }

    fn is_running(&self) -> VmResult<bool> {
        const VMS: &str = "VMState=\"";
        let s = self.show_vm_info()?;
        for x in s.lines() {
            if x.starts_with(VMS) {
                return Ok(&x[VMS.len()..x.len() - 1] == "running");
            }
        }
        vmerr!(ErrorKind::UnexpectedResponse(s))
    }

    fn reboot(&self) -> VmResult<()> {
        self.stop()?;
        loop {
            let status = self.start();
            if status == Ok(()) {
                return Ok(());
            } else if status == vmerr!(ErrorKind::VmIsRunning) {
                // Do nothing
            } else if let Err(x) = status {
                return Err(x);
            }
        }
    }

    fn hard_reboot(&self) -> VmResult<()> {
        self.reset_vm()
    }

    fn pause(&self) -> VmResult<()> {
        Self::pause_vm(self)
    }

    fn unpause(&self) -> VmResult<()> {
        self.resume_vm()
    }
}

impl GuestCmd for VBoxManage {
    fn run_command(&self, guest_args: &[&str]) -> VmResult<()> {
        self.run(guest_args)
    }

    fn copy_from_guest_to_host(&self, from_guest_path: &str, to_host_path: &str) -> VmResult<()> {
        self.copy_from(from_guest_path, to_host_path)
    }

    fn copy_from_host_to_guest(&self, from_host_path: &str, to_guest_path: &str) -> VmResult<()> {
        self.copy_to(from_host_path, to_guest_path)
    }
}

impl SnapshotCmd for VBoxManage {
    fn list_snapshots(&self) -> VmResult<Vec<Snapshot>> {
        Self::list_snapshots(self)
    }

    fn take_snapshot(&self, name: &str) -> VmResult<()> {
        Self::take_snapshot(self, name, None, true)
    }

    fn revert_snapshot(&self, name: &str) -> VmResult<()> {
        self.restore_snapshot(name)
    }

    fn delete_snapshot(&self, name: &str) -> VmResult<()> {
        Self::delete_snapshot(self, name)
    }
}
