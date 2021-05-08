// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
//! [VBoxManage](https://www.virtualbox.org/manual/ch08.html) controller.
use crate::types::*;
use std::{
    process::Command,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct VBoxManage {
    executable_path: String,
    vm_name: Option<String>,
    guest_username: Option<String>,
    guest_password: Option<String>,
    guest_password_file: Option<String>,
    guest_domain: Option<String>,
}

impl Default for VBoxManage {
    fn default() -> Self { Self::new() }
}

#[cfg(windows)]
pub const DEFAULT_VBOXMANAGE_PATH: &str =
    r"C:\Program Files\Oracle\VirtualBox\VBoxManage.exe";
#[cfg(not(windows))]
pub const DEFAULT_VBOXMANAGE_PATH: &str = "vboxmanage";

#[cfg(windows)]
const LINE_FEED: &str = "\r\n";
#[cfg(not(windows))]
const LINE_FEED: &str = "\n";

impl VBoxManage {
    pub fn new() -> Self {
        Self {
            executable_path: DEFAULT_VBOXMANAGE_PATH.to_string(),
            vm_name: None,
            guest_username: None,
            guest_password: None,
            guest_password_file: None,
            guest_domain: None,
        }
    }

    /// Sets the path to VBoxManage.
    pub fn executable_path<T: Into<String>>(&mut self, path: T) -> &mut Self {
        self.executable_path = path.into().trim().to_string();
        self
    }

    pub fn get_executable_path(&self) -> &str { &self.executable_path }

    /// Sets the VM name to be manipulated.
    pub fn vm_name<T: Into<Option<String>>>(
        &mut self,
        vm_name: T,
    ) -> &mut Self {
        self.vm_name = vm_name.into();
        self
    }

    pub fn get_vm_name(&self) -> Option<&str> { self.vm_name.as_deref() }

    /// Sets the guest username for login.
    pub fn guest_username<T: Into<Option<String>>>(
        &mut self,
        guest_username: T,
    ) -> &mut Self {
        self.guest_username = guest_username.into();
        self
    }

    pub fn get_guest_username(&self) -> Option<&str> {
        self.guest_username.as_deref()
    }

    /// Sets the guest password for login.
    pub fn guest_password<T: Into<Option<String>>>(
        &mut self,
        guest_password: T,
    ) -> &mut Self {
        self.guest_password = guest_password.into();
        self
    }

    pub fn get_guest_password(&self) -> Option<&str> {
        self.guest_password.as_deref()
    }

    /// Sets the **absolute** path guest password for login.
    pub fn guest_password_file<T: Into<Option<String>>>(
        &mut self,
        guest_password_file: T,
    ) -> &mut Self {
        self.guest_password_file = guest_password_file.into();
        self
    }

    pub fn get_guest_password_file(&self) -> Option<&str> {
        self.guest_password_file.as_deref()
    }

    /// Sets the guest domain for Windows guests.
    pub fn guest_domain<T: Into<Option<String>>>(
        &mut self,
        guest_domain: T,
    ) -> &mut Self {
        self.guest_domain = guest_domain.into();
        self
    }

    pub fn get_guest_domain(&self) -> Option<&str> {
        self.guest_domain.as_deref()
    }

    fn build_auth(&self) -> Vec<&str> {
        let mut v = Vec::with_capacity(8);
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
        use ErrorKind::*;
        use VmPowerState::*;
        starts_err!(s, "Could not find a registered machine named", VmNotFound);
        starts_err!(s, "Could not find a snapshot named ", SnapshotNotFound);
        starts_err!(
            s,
            "This machine does not have any snapshots",
            SnapshotNotFound
        );
        starts_err!(
            s,
            "The specified user was not able to logon on guest",
            GuestAuthenticationFailed
        );
        starts_err!(
            s,
            "Waiting for guest process failed: The guest execution service is \
             not ready (yet)",
            ServiceIsNotRunning
        );
        starts_err!(
            s,
            "Error starting guest session (current status is:",
            ServiceIsNotRunning
        );
        if s.starts_with("FsObjQueryInfo failed on") || s.starts_with("File ") {
            let s = s.lines().last().unwrap();
            return VmError::from(FileError(
                s[s.rfind(':').unwrap() + 2..].to_string(),
            ));
        }

        if let Some(s) = s.strip_prefix("Invalid machine state: ") {
            starts_err!(s, "PoweredOff", InvalidPowerState(Stopped));
            starts_err!(s, "Paused", InvalidPowerState(Paused));
        }
        if let Some(s) = s.strip_prefix("Machine in invalid state ") {
            starts_err!(s, "1 -- powered off", InvalidPowerState(Stopped));
            starts_err!(s, "2 -- saved", InvalidPowerState(Suspended));
        }
        if s.ends_with(" is not currently running")
            || s.contains("is not running")
        {
            return VmError::from(ErrorKind::InvalidPowerState(NotRunning));
        }
        if s.lines().next().unwrap().ends_with(
            "is already locked by a session (or being locked or unlocked)",
        ) {
            return VmError::from(ErrorKind::InvalidPowerState(Running));
        }
        VmError::from(Repr::Unknown(format!("Unknown error: {}", s)))
    }

    /// Checks `s` UUID.
    pub fn is_uuid(s: &str) -> bool {
        regex::Regex::new(
            r#"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$"#
        )
            .unwrap()
            .is_match(s)
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
    fn cmd(&self) -> Command { Command::new(&self.executable_path) }

    /// Gets the VBoxManage version.
    pub fn version(&self) -> VmResult<String> {
        Ok(self.exec(self.cmd().arg("-v"))?.trim().to_string())
    }

    /// Gets a list of VMs.
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
        self.exec(self.cmd().args(&[
            "showvminfo",
            &self.get_vm()?,
            "--machinereadable",
        ]))
    }
    fn get_vm(&self) -> VmResult<&str> {
        match &self.vm_name {
            Some(x) => Ok(x),
            None => vmerr!(ErrorKind::VmIsNotSpecified),
        }
    }

    pub fn start_vm(&self) -> VmResult<()> {
        self.exec2(self.cmd().args(&["startvm", self.get_vm()?]))
    }

    pub fn poweroff_vm(&self) -> VmResult<()> {
        self.exec2(self.cmd().args(&["controlvm", &self.get_vm()?, "poweroff"]))
    }

    /// Sends ACPI shutdown signal.
    ///
    /// If the VM is running, this function returns Ok(()) regardless of whether the VM was shut down.
    pub fn acpi_power_button_vm(&self) -> VmResult<()> {
        self.exec2(self.cmd().args(&[
            "controlvm",
            &self.get_vm()?,
            "acpipowerbutton",
        ]))
    }

    pub fn reset_vm(&self) -> VmResult<()> {
        self.exec2(self.cmd().args(&["controlvm", &self.get_vm()?, "reset"]))
    }

    pub fn pause_vm(&self) -> VmResult<()> {
        self.exec2(self.cmd().args(&["controlvm", &self.get_vm()?, "pause"]))
    }

    pub fn resume_vm(&self) -> VmResult<()> {
        self.exec2(self.cmd().args(&["controlvm", &self.get_vm()?, "resume"]))
    }

    pub fn save_state_vm(&self) -> VmResult<()> {
        self.exec2(self.cmd().args(&[
            "controlvm",
            &self.get_vm()?,
            "savestate",
        ]))
    }

    /// Gets a list of snapshots.
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
        let s = self.exec(self.cmd().args(&[
            "snapshot",
            &self.get_vm()?,
            "list",
            "--machinereadable",
        ]))?;
        let mut ret = vec![];
        if s.trim() == "This machine does not have any snapshots" {
            return Ok(ret);
        }
        let mut last_state = State::Init;

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
                return if last_state == State::Desc
                    || last_state == State::DescCont
                {
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
                    _ => {
                        return vmerr!(ErrorKind::UnexpectedResponse(
                            x.to_string()
                        ))
                    }
                },
                State::Name => match now_data {
                    State::Uuid => {
                        let p = x.find('=').expect("Invalid UUID");
                        sn.id = Some(x[p + 2..x.len() - 1].to_string());
                        last_state = State::Uuid;
                    }
                    _ => {
                        return vmerr!(ErrorKind::UnexpectedResponse(
                            x.to_string()
                        ))
                    }
                },
                State::Uuid => match now_data {
                    State::Desc => {
                        let p = x.find('=').expect("Invalid description");
                        cur_detail = x[p + 2..].to_string();
                        last_state = State::Desc;
                    }
                    _ => {
                        return vmerr!(ErrorKind::UnexpectedResponse(
                            x.to_string()
                        ))
                    }
                },
                State::Desc => match now_data {
                    State::Name => {
                        sn.detail = Some(
                            cur_detail[..cur_detail.len() - 1].to_string(),
                        );
                        ret.push(sn.clone());
                        cur_detail = "".to_string();
                        let p = x.find('=').expect("Invalid name");
                        sn.name = Some(x[p + 2..x.len() - 1].to_string());
                        last_state = State::Name;
                    }
                    State::DescCont => {
                        cur_detail += LINE_FEED;
                        cur_detail += x;
                        last_state = State::DescCont;
                    }
                    _ => {
                        return vmerr!(ErrorKind::UnexpectedResponse(
                            x.to_string()
                        ))
                    }
                },
                State::DescCont => match now_data {
                    State::Name => {
                        sn.detail = Some(
                            cur_detail[..cur_detail.len() - 1].to_string(),
                        );
                        ret.push(sn.clone());
                        cur_detail = "".to_string();
                        let p = x.find('=').expect("Invalid name");
                        sn.name = Some(x[p + 2..x.len() - 1].to_string());
                        last_state = State::Name;
                    }
                    State::DescCont => {
                        cur_detail += LINE_FEED;
                        cur_detail += x;
                        last_state = State::DescCont;
                    }
                    _ => {
                        return vmerr!(ErrorKind::UnexpectedResponse(
                            x.to_string()
                        ))
                    }
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
        cmd.args(&["snapshot", &self.get_vm()?, "take", name]);
        if let Some(x) = description {
            cmd.args(&["--description", x]);
        }
        if is_live {
            cmd.arg("--live");
        }
        self.exec2(&mut cmd)
    }

    pub fn delete_snapshot(&self, name: &str) -> VmResult<()> {
        self.exec2(self.cmd().args(&[
            "snapshot",
            &self.get_vm()?,
            "delete",
            name,
        ]))
    }

    pub fn restore_snapshot(&self, name: &str) -> VmResult<()> {
        self.exec2(self.cmd().args(&[
            "snapshot",
            &self.get_vm()?,
            "restore",
            name,
        ]))
    }

    pub fn restore_current_snapshot(&self) -> VmResult<()> {
        self.exec2(self.cmd().args(&[
            "snapshot",
            &self.get_vm()?,
            "restorecurrent",
        ]))
    }

    pub fn run(&self, guest_args: &[&str]) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["guestcontrol", &self.get_vm()?, "run"]);
        cmd.args(self.build_auth());
        cmd.args(guest_args);
        self.exec2(&mut cmd)
    }

    /// Copies a file from guest to host.
    pub fn copy_from(
        &self,
        from_guest_path: &str,
        to_host_path: &str,
    ) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["guestcontrol", &self.get_vm()?, "copyfrom"]);
        cmd.args(self.build_auth());
        cmd.args(&[from_guest_path, to_host_path]);
        self.exec2(&mut cmd)
    }

    /// Copies a file from host to guest.
    pub fn copy_to(
        &self,
        from_host_path: &str,
        to_guest_path: &str,
    ) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["guestcontrol", &self.get_vm()?, "copyto"]);
        cmd.args(self.build_auth());
        cmd.args(&[from_host_path, to_guest_path]);
        self.exec2(&mut cmd)
    }

    /// Sends keyboard scancodes to the guest.
    pub fn keyboard_put_scancode<T: Iterator<Item = u8>>(
        &self,
        v: T,
    ) -> VmResult<()> {
        use std::fmt::Write;
        let mut cmd = self.cmd();
        cmd.args(&["controlvm", &self.get_vm()?, "keyboardputscancode"]);
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
        cmd.args(&["controlvm", &self.get_vm()?, "keyboardputstring"]);
        cmd.args(self.build_auth());
        cmd.args(v);
        self.exec2(&mut cmd)
    }
}

impl PowerCmd for VBoxManage {
    fn start(&self) -> VmResult<()> { self.start_vm() }

    /// Sends ACPI shutdown signals.
    fn stop<D: Into<Option<Duration>>>(&self, timeout: D) -> VmResult<()> {
        let timeout = timeout.into();
        let s = Instant::now();
        let mut ok_flag = false;
        loop {
            if let Some(timeout) = timeout {
                if s.elapsed() >= timeout {
                    return vmerr!(ErrorKind::Timeout);
                }
            }

            match self.acpi_power_button_vm() {
                Ok(()) => {
                    ok_flag = true;
                }
                Err(x) => {
                    if let Some(is_running) = x.is_invalid_state_running() {
                        if !is_running {
                            // !InvalidVmState(Running)
                            return if ok_flag { Ok(()) } else { Err(x) };
                        }
                    } else {
                        return Err(x);
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    fn hard_stop(&self) -> VmResult<()> {
        let mut ok_flag = false;
        loop {
            match self.poweroff_vm() {
                Ok(()) => {
                    ok_flag = true;
                }
                Err(x) => {
                    match x.get_invalid_state() {
                        Some(VmPowerState::Stopped) => { /* Does nothing */ }
                        Some(VmPowerState::NotRunning) => {
                            return if ok_flag { Ok(()) } else { Err(x) }
                        }
                        _ => return Err(x),
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    fn suspend(&self) -> VmResult<()> {
        let mut ok_flag = false;
        loop {
            // NotRunningが返ってきたらSuspendに成功した証なのだが、
            //
            let status = self.save_state_vm();
            match status {
                Ok(_) => {
                    ok_flag = true;
                }
                Err(x) => {
                    match x.get_invalid_state() {
                        Some(VmPowerState::Suspended) => { /* Does nothing */ }
                        Some(VmPowerState::NotRunning) => {
                            return if ok_flag { Ok(()) } else { Err(x) }
                        }
                        _ => return Err(x),
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    fn resume(&self) -> VmResult<()> { self.start_vm() }

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

    fn reboot<D: Into<Option<Duration>>>(&self, timeout: D) -> VmResult<()> {
        self.stop(timeout)?;
        loop {
            match self.start() {
                Ok(()) => return Ok(()),
                Err(x) => {
                    if x.is_invalid_state_running() == Some(true) { /* Does nothing */
                    } else {
                        return Err(x);
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    fn hard_reboot(&self) -> VmResult<()> { self.reset_vm() }

    fn pause(&self) -> VmResult<()> { Self::pause_vm(self) }

    fn unpause(&self) -> VmResult<()> { self.resume_vm() }
}

impl GuestCmd for VBoxManage {
    fn exec_cmd(&self, guest_args: &[&str]) -> VmResult<()> {
        self.run(guest_args)
    }

    fn copy_from_guest_to_host(
        &self,
        from_guest_path: &str,
        to_host_path: &str,
    ) -> VmResult<()> {
        self.copy_from(from_guest_path, to_host_path)
    }

    fn copy_from_host_to_guest(
        &self,
        from_host_path: &str,
        to_guest_path: &str,
    ) -> VmResult<()> {
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
