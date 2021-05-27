// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
//! [VBoxManage](https://www.virtualbox.org/manual/ch08.html) controller.
use crate::{exec_cmd, types::*};
use std::{
    collections::HashMap,
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

    impl_setter!(
        /// Sets the path to VBoxManage.
        executable_path: String
    );

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

    impl_setter!(@opt
    /// Sets the guest username for login.
        guest_username: String
    );

    pub fn get_guest_username(&self) -> Option<&str> {
        self.guest_username.as_deref()
    }

    impl_setter!(@opt
    /// Sets the guest password for login.
        guest_password: String
    );

    pub fn get_guest_password(&self) -> Option<&str> {
        self.guest_password.as_deref()
    }

    impl_setter!(@opt
    /// Sets the absolute path to the guest password file for login.
        guest_password_file: String
    );

    pub fn get_guest_password_file(&self) -> Option<&str> {
        self.guest_password_file.as_deref()
    }

    impl_setter!(@opt
    /// Sets the guest domain for Windows guests.
        guest_domain: String
    );

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

        if let Some(s) = s.strip_prefix("RTPathQueryInfo failed on ") {
            return VmError::from(FileError(s.to_string()));
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

    fn exec(cmd: &mut Command) -> VmResult<String> {
        let (stdout, stderr) = exec_cmd(cmd)?;
        if !stderr.is_empty() {
            Self::check(stderr)
        } else {
            Ok(stdout)
        }
    }

    #[inline]
    fn cmd(&self) -> Command { Command::new(&self.executable_path) }

    /// Gets the VBoxManage version.
    pub fn version(&self) -> VmResult<String> {
        Ok(Self::exec(self.cmd().arg("-v"))?.trim().to_string())
    }

    /// Gets a list of VMs.
    pub fn list_vms(&self) -> VmResult<Vec<Vm>> {
        let s = Self::exec(self.cmd().args(&["list", "vms"]))?;
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
        self.show_vm_info2(&self.get_vm()?)
    }

    pub fn get_os_version(&self) -> VmResult<String> {
        let s = self.show_vm_info()?;
        let hm = Self::parse_info(&s, Some("Guest OS"));
        Ok(hm["ostype"].to_string())
    }

    fn parse_info<'a>(
        s: &'a str,
        stopper: Option<&str>,
    ) -> HashMap<&'a str, &'a str> {
        let l = s.lines();
        let mut hm = HashMap::new();
        for x in l {
            let x: Vec<&str> = x.splitn(2, '=').collect();
            if x.len() != 2 {
                continue;
            }
            let (key, value) = (x[0].trim(), x[1].trim());
            let value = if value.len() >= 2 {
                if value.starts_with('"') && value.ends_with('"') {
                    // strip ""
                    &value[1..value.len() - 1]
                } else {
                    value
                }
            } else {
                value
            };
            hm.insert(key, value);
            if let Some(stopper) = stopper {
                if key == stopper {
                    break;
                }
            }
        }
        hm
    }

    fn show_vm_info2(&self, id: &str) -> VmResult<String> {
        Self::exec(self.cmd().args(&["showvminfo", id, "--machinereadable"]))
    }

    fn get_vm(&self) -> VmResult<&str> {
        self.vm_name
            .as_deref()
            .ok_or_else(|| VmError::from(ErrorKind::VmIsNotSpecified))
    }

    pub fn start_vm(&self) -> VmResult<()> {
        Self::exec(self.cmd().args(&["startvm", self.get_vm()?]))?;
        Ok(())
    }

    pub fn poweroff_vm(&self) -> VmResult<()> {
        Self::exec(self.cmd().args(&[
            "controlvm",
            &self.get_vm()?,
            "poweroff",
        ]))?;
        Ok(())
    }

    /// Sends ACPI shutdown signal.
    ///
    /// If the VM is running, this function returns Ok(()) regardless of whether the VM was shut down.
    pub fn acpi_power_button_vm(&self) -> VmResult<()> {
        Self::exec(self.cmd().args(&[
            "controlvm",
            &self.get_vm()?,
            "acpipowerbutton",
        ]))?;
        Ok(())
    }

    pub fn reset_vm(&self) -> VmResult<()> {
        Self::exec(self.cmd().args(&["controlvm", &self.get_vm()?, "reset"]))?;
        Ok(())
    }

    pub fn pause_vm(&self) -> VmResult<()> {
        Self::exec(self.cmd().args(&["controlvm", &self.get_vm()?, "pause"]))?;
        Ok(())
    }

    pub fn resume_vm(&self) -> VmResult<()> {
        Self::exec(self.cmd().args(&["controlvm", &self.get_vm()?, "resume"]))?;
        Ok(())
    }

    pub fn save_state_vm(&self) -> VmResult<()> {
        Self::exec(self.cmd().args(&[
            "controlvm",
            &self.get_vm()?,
            "savestate",
        ]))?;
        Ok(())
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
        let s = Self::exec(self.cmd().args(&[
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
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn delete_snapshot(&self, name: &str) -> VmResult<()> {
        Self::exec(self.cmd().args(&[
            "snapshot",
            &self.get_vm()?,
            "delete",
            name,
        ]))?;
        Ok(())
    }

    pub fn restore_snapshot(&self, name: &str) -> VmResult<()> {
        Self::exec(self.cmd().args(&[
            "snapshot",
            &self.get_vm()?,
            "restore",
            name,
        ]))?;
        Ok(())
    }

    pub fn restore_current_snapshot(&self) -> VmResult<()> {
        Self::exec(self.cmd().args(&[
            "snapshot",
            &self.get_vm()?,
            "restorecurrent",
        ]))?;
        Ok(())
    }

    pub fn run(&self, guest_args: &[&str]) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["guestcontrol", &self.get_vm()?, "run"]);
        cmd.args(self.build_auth());
        cmd.args(guest_args);
        Self::exec(&mut cmd)?;
        Ok(())
    }

    /// Copies files from guest to host.
    pub fn copy_from(
        &self,
        follow: bool,
        recursive: bool,
        from_guest_paths: &[&str],
        to_host_path: &str,
    ) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["guestcontrol", &self.get_vm()?, "copyfrom"]);
        cmd.args(self.build_auth());
        if follow {
            cmd.arg("--follow");
        }
        if recursive {
            cmd.arg("-R");
        }

        cmd.args(from_guest_paths);
        cmd.arg(to_host_path);
        Self::exec(&mut cmd)?;
        Ok(())
    }

    /// Copies files from host to guest.
    pub fn copy_to(
        &self,
        follow: bool,
        recursive: bool,
        from_host_paths: &[&str],
        to_guest_path: &str,
    ) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["guestcontrol", &self.get_vm()?, "copyto"]);
        cmd.args(self.build_auth());
        if follow {
            cmd.arg("--follow");
        }
        if recursive {
            cmd.arg("-R");
        }
        cmd.args(from_host_paths);
        cmd.arg(to_guest_path);
        Self::exec(&mut cmd)?;
        Ok(())
    }

    /// Remove files from guest.
    pub fn remove_file(&self, guest_paths: &[&str]) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["guestcontrol", &self.get_vm()?, "rm"]);
        cmd.args(self.build_auth());
        cmd.arg("-f");
        cmd.args(guest_paths);
        Self::exec(&mut cmd)?;
        Ok(())
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
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn keyboard_put_string(&self, v: &[&str]) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["controlvm", &self.get_vm()?, "keyboardputstring"]);
        cmd.args(self.build_auth());
        cmd.args(v);
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn install_ext_pack(
        &self,
        replace: bool,
        accept_license: bool,
        ext_pack_path: &str,
    ) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["extpack", "install"]);
        if replace {
            cmd.arg("--replace");
        }
        if accept_license {
            cmd.arg("--accept-license=sha256");
        }
        cmd.arg(ext_pack_path);
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn uninstall_ext_pack(
        &self,
        force: bool,
        ext_pack_path: &str,
    ) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["extpack", "uninstall"]);
        if force {
            cmd.arg("--force");
        }
        cmd.arg(ext_pack_path);
        Self::exec(&mut cmd)?;
        Ok(())
    }

    pub fn cleanup_ext_pack(&self) -> VmResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&["extpack", "cleanup"]);
        Self::exec(&mut cmd)?;
        Ok(())
    }
}

impl VmCmd for VBoxManage {
    fn list_vms(&self) -> VmResult<Vec<Vm>> { self.list_vms() }

    fn set_vm_by_id(&mut self, id: &str) -> VmResult<()> {
        // VBoxManage can be passed an ID.
        self.set_vm_by_name(id)
    }

    fn set_vm_by_name(&mut self, name: &str) -> VmResult<()> {
        self.show_vm_info2(name)?; // Checks if the corresponding VM exists.
        self.vm_name = Some(name.to_string());
        Ok(())
    }

    /// `path` is the absolute path of a `vbox` file.
    fn set_vm_by_path(&mut self, path: &str) -> VmResult<()> {
        use ErrorKind::UnexpectedResponse;
        // `\` in CfgFile of show_vm_info is escaped, So `path` also needs to be escaped.
        let path = path.replace('\\', "\\\\");
        let vms = self.list_vms()?;
        // VBoxManage's machine readable format collapses if the snapshot detail contains `"` or `\`.
        // To avoid this, call show_vm_info multiple times (it takes time).
        for vm in vms {
            let id = vm.id.as_ref().unwrap();
            let s = self.show_vm_info2(id)?;
            let s2 = s.clone();
            let cfg_path = s
                .lines()
                .nth(4)
                .ok_or_else(|| VmError::from(UnexpectedResponse(s2.clone())))?
                .strip_prefix("CfgFile=\"")
                .ok_or_else(|| VmError::from(UnexpectedResponse(s2.clone())))?;
            let cfg_path = &cfg_path[..cfg_path.len() - 1];
            if path == cfg_path {
                self.vm_name = Some(id.to_string());
                return Ok(());
            }
        }
        vmerr!(ErrorKind::VmNotFound)
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

            if let Some(timeout) = timeout {
                if s.elapsed() >= timeout {
                    return vmerr!(ErrorKind::Timeout);
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
        self.copy_from(false, true, &[from_guest_path], to_host_path)
    }

    fn copy_from_host_to_guest(
        &self,
        from_host_path: &str,
        to_guest_path: &str,
    ) -> VmResult<()> {
        self.copy_to(false, true, &[from_host_path], to_guest_path)
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
