// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
//! Hyper-V cmdlets controller.
//!
//! Note: [In Windows Server 2012 R2, virtual machine snapshots were renamed to virtual machine checkpoints](https://docs.microsoft.com/en-us/previous-versions/windows/it-pro/windows-server-2012-r2-and-2012/dn818483(v=ws.11))
use crate::{deserialize, exec_cmd_astr, types::*};
use serde::Deserialize;
use std::{ffi::OsStr, process::Command, time::Duration};

/// Escapes an argument.
///
/// Surrounds the argument with single quotes and escapes single quotes.
pub fn escape_pwsh<S: AsRef<str>>(s: S) -> String {
    let s = s.as_ref();
    let mut ret = String::with_capacity(s.as_bytes().len() + 2);
    ret.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            ret.push('\'');
        }
        ret.push(ch);
    }
    ret.push('\'');
    ret
}

/// Represents Hyper-V powershell command executor.
#[derive(Clone, Debug)]
pub struct HyperVCmd {
    executable_path: String,
    vm_name: Option<String>,
    guest_username: Option<String>,
    guest_password: Option<String>,
}

impl Default for HyperVCmd {
    fn default() -> Self {
        Self {
            executable_path: "powershell".to_string(),
            vm_name: None,
            guest_username: None,
            guest_password: None,
        }
    }
}

struct PsCommand {
    cmd: Command,
    cmdlet_name: &'static str,
}

impl PsCommand {
    fn new(pwsh_path: &str, cmdlet_name: &'static str) -> Self {
        let mut cmd = Command::new(pwsh_path);
        cmd.args(&[
            "-NoProfile",
            "-NoLogo",
            "-Command",
            "[Threading.Thread]::CurrentThread.CurrentUICulture = 'en-US';", // Make the exception message English.
        ]);
        cmd.arg(cmdlet_name);
        PsCommand { cmd, cmdlet_name }
    }

    fn new_with_session(
        pwsh_path: &str,
        cmdlet_name: &'static str,
        vm: &str,
        username: &str,
        password: &str,
    ) -> Self {
        let mut cmd = Command::new(pwsh_path);
        cmd.args(&[
            "-NoProfile",
            "-NoLogo",
            "-Command",
            "[Threading.Thread]::CurrentThread.CurrentUICulture = 'en-US';", // Make the exception message English.
        ]);
        let mut psc = PsCommand { cmd, cmdlet_name };
        psc.create_session(vm, username, password);
        psc.cmd.arg(cmdlet_name);
        psc
    }

    fn create_session(
        &mut self,
        vm: &str,
        username: &str,
        password: &str,
    ) -> &mut Self {
        self.cmd.args(&[
            "$password = ConvertTo-SecureString",
            password,
            "-AsPlainText -Force;",
        ]);
        self.cmd.args(&[
            "$cred = New-Object System.Management.Automation.PSCredential (",
            username,
            ", $password);",
        ]);
        self.cmd.args(&[
            "$sess = New-PSSession -VMName",
            vm,
            "-Credential $cred;",
        ]);
        self
    }

    fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        self.cmd.arg(arg);
        self
    }

    fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.cmd.args(args);
        self
    }

    unsafe fn arg_array_unescaped<I>(&mut self, arr: I) -> &mut Self
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        self.cmd.arg("@(");
        self.cmd.args(arr);
        self.cmd.arg(")");
        self
    }

    fn exec(&mut self) -> VmResult<String> {
        let (stdout, stderr) = exec_cmd_astr(&mut self.cmd)?;
        if !stderr.is_empty() {
            Self::check(stderr, self.cmdlet_name)
        } else {
            Ok(stdout)
        }
    }

    #[inline]
    fn check(s: String, cmd_name: &str) -> VmResult<String> {
        let error_str = format!("{} : ", cmd_name);
        if let Some(s) = s.strip_prefix(&error_str) {
            Err(Self::handle_error(s.trim()))
        } else {
            Ok(s)
        }
    }

    #[inline]
    fn handle_error(s: &str) -> VmError {
        const IP: &str = "Cannot validate argument on parameter '";
        starts_err!(
            s,
            "You do not have the required permission to complete this task.",
            ErrorKind::PrivilegesRequired
        );
        starts_err!(
            s,
            "Hyper-V was unable to find a virtual machine with name",
            ErrorKind::VmNotFound
        );
        starts_err!(
            s,
            "The operation cannot be performed while the virtual machine is \
             in its current state.",
            ErrorKind::InvalidPowerState(VmPowerState::Unknown)
        );
        starts_err!(
            s,
            "Unable to find a snapshot matching the given criteria.",
            ErrorKind::SnapshotNotFound
        );
        if let Some(s) = s.strip_prefix("Access to the path") {
            if s.contains(" is denied.") {
                return VmError::from(ErrorKind::PermissionDenied);
            }
            return VmError::from(ErrorKind::UnexpectedResponse(s.to_string()));
        }
        if let Some(s) = s.strip_prefix(IP) {
            let p = s.find("'.").unwrap();
            return VmError::from(ErrorKind::InvalidParameter(
                s[IP.len()..IP.len() + p].to_string(),
            ));
        }
        VmError::from(Repr::Unknown(format!("Unknown error: {}", s)))
    }
}
impl HyperVCmd {
    pub fn new() -> Self { Self::default() }

    impl_setter!(
        /// Sets the path to PowerShell.
        executable_path: String
    );

    pub fn get_executable_path(&self) -> &str { &self.executable_path }

    pub fn vm_name<T: Into<Option<String>>>(
        &mut self,
        vm_name: T,
    ) -> &mut Self {
        self.vm_name = vm_name.into().map(escape_pwsh);
        self
    }

    pub fn guest_username<T: Into<Option<String>>>(
        &mut self,
        guest_username: T,
    ) -> &mut Self {
        self.guest_username = guest_username.into().map(escape_pwsh);
        self
    }

    pub fn guest_password<T: Into<Option<String>>>(
        &mut self,
        guest_password: T,
    ) -> &mut Self {
        self.guest_password = guest_password.into().map(escape_pwsh);
        self
    }

    pub fn get_vm_name(&self) -> Option<&str> { self.vm_name.as_deref() }

    fn retrieve_vm(&self) -> VmResult<&str> {
        // self.vm_name is escaped on input.
        self.vm_name
            .as_deref()
            .ok_or_else(|| VmError::from(ErrorKind::VmIsNotSpecified))
    }

    fn retrieve_username(&self) -> VmResult<&str> {
        // self.username is escaped on input.
        self.guest_username
            .as_deref()
            .ok_or_else(|| VmError::from(ErrorKind::CredentialIsNotSpecified))
    }

    fn retrieve_password(&self) -> VmResult<&str> {
        // self.password is escaped on input.
        self.guest_password
            .as_deref()
            .ok_or_else(|| VmError::from(ErrorKind::CredentialIsNotSpecified))
    }

    fn deserialize_resp<'a, T: Deserialize<'a>>(
        s: &'a str,
    ) -> VmResult<Vec<T>> {
        if s.starts_with('[') {
            // `s` is an array.
            deserialize::<Vec<T>>(s)
        } else {
            // `s` is a dictionary element.
            Ok(vec![deserialize::<T>(s)?])
        }
    }
}

impl VmCmd for HyperVCmd {
    fn list_vms(&self) -> VmResult<Vec<Vm>> {
        raw::get_vm(&self.executable_path)
    }

    /// `id` is VMId which can be obtained with `Get-VM|select VMId`.
    fn set_vm_by_id(&mut self, id: &str) -> VmResult<()> {
        for vm in self.list_vms()? {
            if id == vm.id.as_deref().expect("VMId does not exist") {
                self.vm_name(vm.name);
                return Ok(());
            }
        }
        vmerr!(ErrorKind::VmNotFound)
    }

    fn set_vm_by_name(&mut self, name: &str) -> VmResult<()> {
        for vm in self.list_vms()? {
            if name == vm.name.as_deref().expect("Name does not exist") {
                self.vm_name(vm.name);
                return Ok(());
            }
        }
        vmerr!(ErrorKind::VmNotFound)
    }

    /// Due to the specification of Hyper-V, HyperVCmd does not support this function.
    fn set_vm_by_path(&mut self, _: &str) -> VmResult<()> {
        vmerr!(ErrorKind::UnsupportedCommand)
    }
}

impl PowerCmd for HyperVCmd {
    fn start(&self) -> VmResult<()> {
        unsafe {
            raw_unescaped::start_vm_unescaped(
                &self.executable_path,
                &[self.retrieve_vm()?],
            )
        }
    }

    fn stop<D: Into<Option<Duration>>>(&self, _timeout: D) -> VmResult<()> {
        unsafe {
            raw_unescaped::stop_vm_unescaped(
                &self.executable_path,
                &[self.retrieve_vm()?],
                false,
                false,
            )
        }
    }

    fn hard_stop(&self) -> VmResult<()> {
        unsafe {
            raw_unescaped::stop_vm_unescaped(
                &self.executable_path,
                &[self.retrieve_vm()?],
                true,
                false,
            )
        }
    }

    fn suspend(&self) -> VmResult<()> {
        unsafe {
            raw_unescaped::suspend_vm_unescaped(
                &self.executable_path,
                &[self.retrieve_vm()?],
            )
        }
    }
    fn resume(&self) -> VmResult<()> {
        unsafe {
            raw_unescaped::resume_vm_unescaped(
                &self.executable_path,
                &[self.retrieve_vm()?],
            )
        }
    }

    fn is_running(&self) -> VmResult<bool> {
        unsafe {
            Ok(raw_unescaped::get_power_state_unescaped(
                &self.executable_path,
                self.retrieve_vm()?,
            )? == VmPowerState::Running)
        }
    }

    fn reboot<D: Into<Option<Duration>>>(&self, timeout: D) -> VmResult<()> {
        self.stop(timeout)?;
        self.start()
    }

    fn hard_reboot(&self) -> VmResult<()> {
        self.hard_stop()?;
        self.start()
    }

    fn pause(&self) -> VmResult<()> { self.suspend() }

    fn unpause(&self) -> VmResult<()> { self.resume() }
}

#[test]
fn test_escape_pwsh() {
    assert_eq!("''''''''", escape_pwsh("'''"));
    assert_eq!("'MSEdge - Win10'", escape_pwsh("MSEdge - Win10"));
    assert_eq!("'\"MSEdge - Win10\"'", escape_pwsh("\"MSEdge - Win10\""));
    assert_eq!(
        "'MSEdge - Win10'';calc.exe #'",
        escape_pwsh("MSEdge - Win10';calc.exe #")
    );
    assert_eq!("'MSEdge - Win10`'", escape_pwsh("MSEdge - Win10`"));
    assert_eq!("'MSEdge - $a`'", escape_pwsh("MSEdge - $a`"));
}

impl SnapshotCmd for HyperVCmd {
    fn list_snapshots(&self) -> VmResult<Vec<Snapshot>> {
        unsafe {
            raw_unescaped::get_vm_snapshot_unescaped(
                &self.executable_path,
                self.retrieve_vm()?,
            )
        }
    }

    fn take_snapshot(&self, name: &str) -> VmResult<()> {
        unsafe {
            raw_unescaped::checkpoint_vm_unescaped(
                &self.executable_path,
                &[self.retrieve_vm()?],
                &escape_pwsh(name),
            )
        }
    }

    fn revert_snapshot(&self, name: &str) -> VmResult<()> {
        unsafe {
            raw_unescaped::restore_vm_snapshot_unescaped(
                &self.executable_path,
                self.retrieve_vm()?,
                &escape_pwsh(name),
            )
        }
    }

    fn delete_snapshot(&self, name: &str) -> VmResult<()> {
        // Remove-VMSnapshot does not change the response regardless of whether a snapshot exists or not.
        let sn = self.list_snapshots()?;
        if !sn.iter().any(|x| x.name.as_deref() == Some(name)) {
            // The snapshot named `name` doesn't exist.
            return vmerr!(ErrorKind::SnapshotNotFound);
        }
        unsafe {
            raw_unescaped::remove_vm_snapshot_unescaped(
                &self.executable_path,
                &[self.retrieve_vm()?],
                &escape_pwsh(name),
            )
        }
    }
}

impl GuestCmd for HyperVCmd {
    fn exec_cmd(&self, _guest_args: &[&str]) -> VmResult<()> {
        unimplemented!("exec_cmd of HyperVCmd is not implemented")
    }

    fn copy_from_guest_to_host(
        &self,
        from_guest_path: &str,
        to_host_path: &str,
    ) -> VmResult<()> {
        unsafe {
            raw_unescaped::copy_vm_file_from_guest_to_host_unescaped(
                &self.executable_path,
                self.retrieve_vm()?,
                &escape_pwsh(from_guest_path),
                &escape_pwsh(to_host_path),
                self.retrieve_username()?,
                self.retrieve_password()?,
            )
        }
    }

    fn copy_from_host_to_guest(
        &self,
        from_host_path: &str,
        to_guest_path: &str,
    ) -> VmResult<()> {
        unsafe {
            raw_unescaped::copy_vm_file_unescaped(
                &self.executable_path,
                &[self.retrieve_vm()?],
                &escape_pwsh(from_host_path),
                &escape_pwsh(to_guest_path),
                true,
            )
        }
    }
}

#[repr(u8)]
/// Represents `[Microsoft.HyperV.Powershell.VMOperationalStatus]`.
pub enum PowerShellVmState {
    Other = 1,
    Running,
    Off,
    Stopping,
    Saved,
    Paused,
    Starting,
    Reset,
    Saving,
    Pausing,
    Resuming,
    FastSaved,
    FastSaving,
    ForceShutdown,
    ForceReboot,
    Hibernated,
    ComponentServicing,
    RunningCritical,
    OffCritical,
    StoppingCritical,
    SavedCritical,
    PausedCritical,
    StartingCritical,
    ResetCritical,
    SavingCritical,
    PausingCritical,
    ResumingCritical,
    FastSavedCritical,
    FastSavingCritical,
}

pub mod raw {
    use crate::{
        hyperv::{escape_pwsh, hypervcmd::PsCommand, raw_unescaped, HyperVCmd},
        types::*,
        VmResult,
    };
    use serde::Deserialize;
    use std::ffi::OsStr;
    /// Gets a list of VMs.
    pub fn get_vm(pwsh_path: &str) -> VmResult<Vec<Vm>> {
        let s = PsCommand::new(pwsh_path, "Get-VM")
            .arg("|select VMId, Name|ConvertTo-Json")
            .exec()?;
        #[derive(Deserialize)]
        struct Response {
            #[serde(alias = "VMId")]
            id: String,
            #[serde(alias = "Name")]
            name: String,
        }
        if s.is_empty() {
            // No snapshot.
            return Ok(vec![]);
        }
        let resp = HyperVCmd::deserialize_resp::<Response>(&s)?;
        Ok(resp
            .iter()
            .map(|x| Vm {
                id: Some(x.id.clone()),
                name: Some(x.name.clone()),
                path: None,
            })
            .collect())
    }

    /// Gets a power state of the VM.
    pub fn get_power_state(
        pwsh_path: &str,
        vm: &str,
    ) -> VmResult<VmPowerState> {
        unsafe {
            raw_unescaped::get_power_state_unescaped(
                pwsh_path,
                &escape_pwsh(vm),
            )
        }
    }

    /// Starts VMs.
    ///
    /// For more information, See [Start-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/start-vm).
    pub fn start_vm(pwsh_path: &str, vms: &[&str]) -> VmResult<()> {
        unsafe {
            raw_unescaped::start_vm_unescaped(
                pwsh_path,
                vms.iter().map(escape_pwsh),
            )
        }
    }

    /// Restarts VMs.
    ///
    /// For more information, See [Restart-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/restart-vm).
    pub fn restart_vm(pwsh_path: &str, vms: &[&str]) -> VmResult<()> {
        unsafe {
            raw_unescaped::restart_vm_unchecked(
                pwsh_path,
                vms.iter().map(escape_pwsh),
            )
        }
    }

    /// Stops VMs.
    ///
    /// For more information, See [Stop-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/stop-vm).
    pub fn stop_vm(
        pwsh_path: &str,
        vms: &[&str],
        turn_off: bool,
        use_save: bool,
    ) -> VmResult<()> {
        unsafe {
            raw_unescaped::stop_vm_unescaped(
                pwsh_path,
                vms.iter().map(escape_pwsh),
                turn_off,
                use_save,
            )
        }
    }

    /// Suspends VMs.
    ///
    /// For more information, See [Suspend-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/suspend-vm).
    pub fn suspend_vm(pwsh_path: &str, vms: &[&str]) -> VmResult<()> {
        unsafe {
            raw_unescaped::suspend_vm_unescaped(
                pwsh_path,
                vms.iter().map(escape_pwsh),
            )
        }
    }

    /// Resumes VMs.
    ///
    /// For more information, See [Resume-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/resume-vm).
    pub fn resume_vm(pwsh_path: &str, vms: &[&str]) -> VmResult<()> {
        unsafe {
            raw_unescaped::resume_vm_unescaped(
                pwsh_path,
                vms.iter().map(escape_pwsh),
            )
        }
    }

    /// Copies a file from the host to guests.
    ///
    /// For more information, See [Copy-VMFile](https://docs.microsoft.com/en-us/powershell/module/hyper-v/copy-vmfile).
    pub fn copy_vm_file(
        pwsh_path: &str,
        vms: &[&str],
        src_path: &str,
        dst_path: &str,
        create_full_path: bool,
    ) -> VmResult<()> {
        unsafe {
            raw_unescaped::copy_vm_file_unescaped(
                pwsh_path,
                vms.iter().map(escape_pwsh),
                &escape_pwsh(src_path),
                &escape_pwsh(dst_path),
                create_full_path,
            )
        }
    }

    /// Gets a list of checkpoints of the VM.
    ///
    /// For more information, See [Get-VMSnapshot](https://docs.microsoft.com/en-us/powershell/module/hyper-v/get-vmsnapshot).
    pub fn get_vm_snapshot(
        pwsh_path: &str,
        vm: &str,
    ) -> VmResult<Vec<Snapshot>> {
        unsafe {
            raw_unescaped::get_vm_snapshot_unescaped(
                pwsh_path,
                &escape_pwsh(vm),
            )
        }
    }

    /// Creates a checkpoint named `name` of VMs.
    ///
    /// For more information, See [Checkpoint-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/checkpoint-vm).
    pub fn checkpoint_vm<I>(pwsh_path: &str, vms: I, name: &str) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        unsafe {
            raw_unescaped::checkpoint_vm_unescaped(
                pwsh_path,
                vms.into_iter().map(escape_pwsh),
                &escape_pwsh(name),
            )
        }
    }

    /// Restores a VM checkpoint named `name`.
    ///
    /// For more information, See [Restore-VMSnapshot](https://docs.microsoft.com/ja-jp/powershell/module/hyper-v/restore-vmsnapshot).
    pub fn restore_vm_snapshot(
        pwsh_path: &str,
        vm_name: &str,
        name: &str,
    ) -> VmResult<()> {
        unsafe {
            raw_unescaped::restore_vm_snapshot_unescaped(
                pwsh_path,
                &escape_pwsh(vm_name),
                &escape_pwsh(name),
            )
        }
    }

    /// Removes a VM checkpoint named `name` from VMs.
    ///
    /// For more information, See [Remove-VMSnapshot](https://docs.microsoft.com/ja-jp/powershell/module/hyper-v/remove-vmsnapshot).
    pub fn remove_vm_snapshot<I>(
        pwsh_path: &str,
        vms: I,
        name: &str,
    ) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        unsafe {
            raw_unescaped::remove_vm_snapshot_unescaped(
                pwsh_path,
                vms.into_iter().map(escape_pwsh),
                &escape_pwsh(name),
            )
        }
    }
}

pub mod raw_unescaped {
    use crate::{
        deserialize,
        hyperv::{hypervcmd::PsCommand, *},
        types::*,
        VmResult,
    };
    use serde::Deserialize;
    use std::ffi::OsStr;

    /// Gets a power state of the VM.
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vm`, which can lead to command injection.
    ///
    /// Please be sure to escape `vm` before calling this function.
    pub unsafe fn get_power_state_unescaped(
        pwsh_path: &str,
        vm: &str,
    ) -> VmResult<VmPowerState> {
        let s = PsCommand::new(pwsh_path, "Get-VM")
            .args(&[vm, "|select State|ConvertTo-Json"])
            .exec()?;
        #[derive(Deserialize)]
        struct Response {
            #[serde(alias = "State")]
            state: u8,
        }
        let state = deserialize::<Response>(&s)?.state;
        macro_rules! m {
            ($x:ident) => {
                state == PowerShellVmState::$x as u8
            };
        }
        Ok(if m!(Running) || m!(RunningCritical) {
            VmPowerState::Running
        } else if m!(Off) || m!(OffCritical) {
            VmPowerState::Stopped
        } else if m!(Saved) || m!(SavedCritical) || m!(FastSaved) {
            VmPowerState::Suspended
        } else if m!(Paused) || m!(PausedCritical) {
            VmPowerState::Paused
        } else {
            VmPowerState::Unknown
        })
    }

    /// Starts VMs.
    ///
    /// For more information, See [Start-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/start-vm).
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vms`, which can lead to command injection.
    ///
    /// Please be sure to escape `vms` before calling this function.
    pub unsafe fn start_vm_unescaped<I>(pwsh_path: &str, vms: I) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        let res = PsCommand::new(pwsh_path, "Start-VM")
            .arg_array_unescaped(vms)
            .exec()?;
        if res.starts_with(
            "WARNING: The virtual machine is already in the specified state.",
        ) {
            return vmerr!(ErrorKind::InvalidPowerState(VmPowerState::Running));
        }
        Ok(())
    }

    /// Stops VMs.
    ///
    /// For more information, See [Stop-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/stop-vm).
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vms`, which can lead to command injection.
    ///
    /// Please be sure to escape `vms` before calling this function.
    pub unsafe fn stop_vm_unescaped<I>(
        pwsh_path: &str,
        vms: I,
        turn_off: bool,
        use_save: bool,
    ) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        let mut cmd = PsCommand::new(pwsh_path, "Stop-VM");
        cmd.arg("-Force");
        cmd.arg_array_unescaped(vms);
        if turn_off {
            cmd.arg("-TurnOff");
        }
        if use_save {
            cmd.arg("-Save");
        }
        let s = cmd.exec()?;
        if s.starts_with(
            "WARNING: The virtual machine is already in the specified state.",
        ) {
            return vmerr!(ErrorKind::InvalidPowerState(VmPowerState::Stopped));
        }
        Ok(())
    }

    /// Suspends VMs.
    ///
    /// For more information, See [Suspend-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/suspend-vm).
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vms`, which can lead to command injection.
    ///
    /// Please be sure to escape `vms` before calling this function.
    pub unsafe fn suspend_vm_unescaped<I>(
        pwsh_path: &str,
        vms: I,
    ) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        let res = PsCommand::new(pwsh_path, "Suspend-VM")
            .arg_array_unescaped(vms)
            .exec()?;
        if res.starts_with(
            "WARNING: The virtual machine is already in the specified state.",
        ) {
            return vmerr!(ErrorKind::InvalidPowerState(
                VmPowerState::Suspended
            ));
        }
        Ok(())
    }

    /// Resumes VMs.
    ///
    /// For more information, See [Resume-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/resume-vm).
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vms`, which can lead to command injection.
    ///
    /// Please be sure to escape `vms` before calling this function.
    pub unsafe fn resume_vm_unescaped<I>(
        pwsh_path: &str,
        vms: I,
    ) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        let s = PsCommand::new(pwsh_path, "Resume-VM")
            .arg_array_unescaped(vms)
            .exec()?;
        if s.starts_with(
            "WARNING: The virtual machine is already in the specified state.",
        ) {
            return vmerr!(ErrorKind::InvalidPowerState(VmPowerState::Running));
        }
        Ok(())
    }

    /// Restarts VMs.
    ///
    /// For more information, See [Restart-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/restart-vm).
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vms`, which can lead to command injection.
    ///
    /// Please be sure to escape `vms` before calling this function.
    pub unsafe fn restart_vm_unchecked<I>(
        pwsh_path: &str,
        vms: I,
    ) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        PsCommand::new(pwsh_path, "Restart-VM")
            .arg("-Confirm:$false")
            .arg_array_unescaped(vms)
            .exec()?;
        Ok(())
    }

    /// Copies a file between from the host to guests.
    ///
    /// For more information, See [Copy-VMFile](https://docs.microsoft.com/en-us/powershell/module/hyper-v/copy-vmfile).
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vms`, `src_path` and `dst_path`, which can lead to command injection.
    ///
    /// Please be sure to escape the parameters before calling this function.
    pub unsafe fn copy_vm_file_unescaped<I>(
        pwsh_path: &str,
        vms: I,
        src_path: &str,
        dst_path: &str,
        create_full_path: bool,
    ) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        let mut cmd = PsCommand::new(pwsh_path, "Copy-VMFile");
        cmd.arg_array_unescaped(vms);
        cmd.args(&[
            "-Force",
            "-SourcePath",
            src_path,
            "-DestinationPath",
            dst_path,
            "-FileSource Host",
        ]);
        if create_full_path {
            cmd.arg("-CreateFullPath");
        }
        cmd.exec()?;
        Ok(())
    }

    /// Copies a file between from a guest to the host with PSSession.
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vms`, `src_path`, `dst_path`, `username` and `password`, which can lead to command injection.
    ///
    /// Please be sure to escape the parameters before calling this function.
    pub unsafe fn copy_vm_file_from_guest_to_host_unescaped(
        pwsh_path: &str,
        vm: &str,
        src_path: &str,
        dst_path: &str,
        username: &str,
        password: &str,
    ) -> VmResult<()> {
        let mut cmd = PsCommand::new_with_session(
            pwsh_path,
            "Copy-Item",
            vm,
            username,
            password,
        );
        cmd.args(&[
            "-FromSession $sess -Path",
            src_path,
            "-Destination",
            dst_path,
            "; Remove-PSSession $sess;",
        ]);
        cmd.exec()?;
        Ok(())
    }

    /// Gets a list of checkpoints of the VM.
    ///
    /// For more information, See [Get-VMSnapshot](https://docs.microsoft.com/en-us/powershell/module/hyper-v/get-vmsnapshot).
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vm`, which can lead to command injection.
    ///
    /// Please be sure to escape the parameters before calling this function.
    pub unsafe fn get_vm_snapshot_unescaped(
        pwsh_path: &str,
        vm: &str,
    ) -> VmResult<Vec<Snapshot>> {
        let s = PsCommand::new(pwsh_path, "Get-VMSnapshot")
            .args(&[vm, "|select Id, Name, Notes|ConvertTo-Json"])
            .exec()?;
        #[derive(Deserialize)]
        struct Response {
            #[serde(alias = "Id")]
            id: String,
            #[serde(alias = "Name")]
            name: String,
            #[serde(alias = "Notes")]
            detail: String,
        }
        if s.is_empty() {
            // No snapshot.
            return Ok(vec![]);
        }
        let resp = HyperVCmd::deserialize_resp::<Response>(&s)?;
        Ok(resp
            .iter()
            .map(|x| Snapshot {
                id: Some(x.id.clone()),
                name: Some(x.name.clone()),
                detail: Some(x.detail.clone()),
            })
            .collect())
    }

    /// Creates a checkpoint named `name` of VMs.
    ///
    /// For more information, See [Checkpoint-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/checkpoint-vm).
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vms` and `name`, which can lead to command injection.
    ///
    /// Please be sure to escape the parameters before calling this function.
    pub unsafe fn checkpoint_vm_unescaped<I>(
        pwsh_path: &str,
        vms: I,
        name: &str,
    ) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        PsCommand::new(pwsh_path, "Checkpoint-VM")
            .arg_array_unescaped(vms)
            .args(&["-SnapshotName", name])
            .exec()?;
        Ok(())
    }

    /// Restores a VM checkpoint named `name`.
    ///
    /// For more information, See [Restore-VMSnapshot](https://docs.microsoft.com/ja-jp/powershell/module/hyper-v/restore-vmsnapshot).
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vm_name` and `name`, which can lead to command injection.
    ///
    /// Please be sure to escape the parameters before calling this function.
    pub unsafe fn restore_vm_snapshot_unescaped(
        pwsh_path: &str,
        vm_name: &str,
        name: &str,
    ) -> VmResult<()> {
        PsCommand::new(pwsh_path, "Restore-VMSnapshot")
            .args(&["-VMName", vm_name, "-Confirm:$false -Name", name])
            .exec()?;
        Ok(())
    }

    /// Removes a VM checkpoint named `name` from VMs.
    ///
    /// For more information, See [Remove-VMSnapshot](https://docs.microsoft.com/en-us/powershell/module/hyper-v/remove-vmsnapshot)
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vms` and `name`, which can lead to command injection.
    ///
    /// Please be sure to escape the parameters before calling this function.
    pub unsafe fn remove_vm_snapshot_unescaped<I>(
        pwsh_path: &str,
        vms: I,
        name: &str,
    ) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        PsCommand::new(pwsh_path, "Remove-VMSnapshot")
            .arg_array_unescaped(vms)
            .args(&["-Confirm:$false -Name", name])
            .exec()?;
        Ok(())
    }
}
