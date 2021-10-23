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
}

impl Default for HyperVCmd {
    fn default() -> Self {
        Self {
            executable_path: "powershell".to_string(),
            vm_name: None,
        }
    }
}

struct PsCommand {
    cmd: Command,
    cmdlet_name: &'static str,
}

impl PsCommand {
    fn new(mut cmd: Command, cmdlet_name: &'static str) -> Self {
        cmd.arg(cmdlet_name);
        PsCommand { cmd, cmdlet_name }
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

    pub fn get_vm_name(&self) -> Option<&str> { self.vm_name.as_deref() }

    fn retrieve_vm(&self) -> VmResult<&str> {
        // self.vm_name is escaped on input.
        self.vm_name
            .as_deref()
            .ok_or_else(|| VmError::from(ErrorKind::VmIsNotSpecified))
    }

    fn cmd(&self, cmdlet: &'static str) -> PsCommand {
        let mut cmd = Command::new(&self.executable_path);
        cmd.args(&[
            "-NoProfile",
            "-NoLogo",
            "[Threading.Thread]::CurrentThread.CurrentUICulture = 'en-US';", // Make the exception message English.
        ]);
        PsCommand::new(cmd, cmdlet)
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

    /// Gets a power state of the VM.
    pub fn get_power_state(&self) -> VmResult<VmPowerState> {
        let s = self
            .cmd("Get-VM")
            .args(&[self.retrieve_vm()?, "|select State|ConvertTo-Json"])
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

    /// Gets a list of VMs.
    pub fn get_vm(&self) -> VmResult<Vec<Vm>> {
        let s = self
            .cmd("Get-VM")
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
        let resp = Self::deserialize_resp::<Response>(&s)?;
        Ok(resp
            .iter()
            .map(|x| Vm {
                id: Some(x.id.clone()),
                name: Some(x.name.clone()),
                path: None,
            })
            .collect())
    }

    /// Starts VMs.
    ///
    /// For more information, See [Start-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/start-vm).
    pub fn start_vm(&self, vms: &[&str]) -> VmResult<()> {
        unsafe { self.start_vm_unescaped(vms.iter().map(escape_pwsh)) }
    }

    /// Starts VMs.
    ///
    /// For more information, See [Start-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/start-vm).
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vms` strings, which can lead to command injection.
    ///
    /// Please be sure to escape `vms` before calling this function.
    pub unsafe fn start_vm_unescaped<I>(&self, vms: I) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        let s = self.cmd("Start-VM").arg_array_unescaped(vms).exec()?;
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
    pub fn restart_vm(&self, vms: &[&str]) -> VmResult<()> {
        unsafe { self.restart_vm_unchecked(vms.iter().map(escape_pwsh)) }
    }

    /// Restarts VMs.
    ///
    /// For more information, See [Restart-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/restart-vm).
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vms` strings, which can lead to command injection.
    ///
    /// Please be sure to escape `vms` before calling this function.
    pub unsafe fn restart_vm_unchecked<I>(&self, vms: I) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        self.cmd("Restart-VM")
            .arg("-Confirm:$false")
            .arg_array_unescaped(vms)
            .exec()?;
        Ok(())
    }

    /// Stops VMs.
    ///
    /// For more information, See [Stop-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/stop-vm).
    pub fn stop_vm(
        &self,
        vms: &[&str],
        turn_off: bool,
        use_save: bool,
    ) -> VmResult<()> {
        unsafe {
            self.stop_vm_unescaped(
                vms.iter().map(escape_pwsh),
                turn_off,
                use_save,
            )
        }
    }

    /// Stops VMs.
    ///
    /// For more information, See [Stop-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/stop-vm).
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vms` strings, which can lead to command injection.
    ///
    /// Please be sure to escape `vms` before calling this function.
    pub unsafe fn stop_vm_unescaped<I>(
        &self,
        vms: I,
        turn_off: bool,
        use_save: bool,
    ) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        let mut cmd = self.cmd("Stop-VM");
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
    pub fn suspend_vm(&self, vms: &[&str]) -> VmResult<()> {
        unsafe { self.suspend_vm_unescaped(vms.iter().map(escape_pwsh)) }
    }

    /// Suspends VMs.
    ///
    /// For more information, See [Suspend-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/suspend-vm).
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vms` strings, which can lead to command injection.
    ///
    /// Please be sure to escape `vms` before calling this function.
    pub unsafe fn suspend_vm_unescaped<I>(&self, vms: I) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        let s = self.cmd("Suspend-VM").arg_array_unescaped(vms).exec()?;
        if s.starts_with(
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
    pub fn resume_vm(&self, vms: &[&str]) -> VmResult<()> {
        unsafe { self.resume_vm_unescaped(vms.iter().map(escape_pwsh)) }
    }

    /// Resumes VMs.
    ///
    /// For more information, See [Resume-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/resume-vm).
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vms` strings, which can lead to command injection.
    ///
    /// Please be sure to escape `vms` before calling this function.
    pub unsafe fn resume_vm_unescaped<I>(&self, vms: I) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        let s = self.cmd("Resume-VM").arg_array_unescaped(vms).exec()?;
        if s.starts_with(
            "WARNING: The virtual machine is already in the specified state.",
        ) {
            return vmerr!(ErrorKind::InvalidPowerState(VmPowerState::Running));
        }
        Ok(())
    }

    /// Copies a file between the host and guests.
    ///
    /// For more information, See [Copy-VMFile](https://docs.microsoft.com/en-us/powershell/module/hyper-v/copy-vmfile).
    pub fn copy_vm_file(
        &self,
        vms: &[&str],
        src_path: &str,
        dst_path: &str,
        create_full_path: bool,
        guest_to_host: bool,
    ) -> VmResult<()> {
        unsafe {
            self.copy_vm_file_unescaped(
                vms.iter().map(escape_pwsh),
                src_path,
                dst_path,
                create_full_path,
                guest_to_host,
            )
        }
    }

    /// Copies a file between the host and guests.
    ///
    /// For more information, See [Copy-VMFile](https://docs.microsoft.com/en-us/powershell/module/hyper-v/copy-vmfile).
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vms` strings, which can lead to command injection.
    ///
    /// Please be sure to escape `vms` before calling this function.
    ///
    /// `src_path` and `dst_path` will be escaped in this function.
    pub unsafe fn copy_vm_file_unescaped<I>(
        &self,
        vms: I,
        src_path: &str,
        dst_path: &str,
        create_full_path: bool,
        guest_to_host: bool,
    ) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        let mut cmd = self.cmd("Copy-VMFile");
        cmd.arg_array_unescaped(vms);
        cmd.args(&[
            "-Force",
            "-SourcePath",
            &escape_pwsh(src_path),
            "-DestinationPath",
            &escape_pwsh(dst_path),
            "-FileSource",
            if guest_to_host { "Guest" } else { "Host" },
        ]);
        if create_full_path {
            cmd.arg("-CreateFullPath");
        }
        cmd.exec()?;
        Ok(())
    }

    /// Gets a list of checkpoints of the VM.
    ///
    /// For more information, See [Get-VMSnapshot](https://docs.microsoft.com/en-us/powershell/module/hyper-v/get-vmsnapshot).
    pub fn get_vm_snapshot(&self) -> VmResult<Vec<Snapshot>> {
        let s = self
            .cmd("Get-VMSnapshot")
            .args(&[
                self.retrieve_vm()?,
                "|select Id, Name, Notes|ConvertTo-Json",
            ])
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
        let resp = Self::deserialize_resp::<Response>(&s)?;
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
    pub fn checkpoint_vm<I>(&self, vms: I, name: &str) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        unsafe {
            self.checkpoint_vm_unescaped(vms.into_iter().map(escape_pwsh), name)
        }
    }

    /// Creates a checkpoint named `name` of VMs.
    ///
    /// For more information, See [Checkpoint-VM](https://docs.microsoft.com/en-us/powershell/module/hyper-v/checkpoint-vm).
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vms` strings, which can lead to command injection.
    ///
    /// Please be sure to escape `vms` before calling this function.
    ///
    /// `name` will be escaped in this function.
    pub unsafe fn checkpoint_vm_unescaped<I>(
        &self,
        vms: I,
        name: &str,
    ) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        self.cmd("Checkpoint-VM")
            .arg_array_unescaped(vms)
            .args(&["-SnapshotName", &escape_pwsh(name)])
            .exec()?;
        Ok(())
    }

    /// Restores a VM checkpoint named `name`.
    ///
    /// For more information, See [Restore-VMSnapshot](https://docs.microsoft.com/ja-jp/powershell/module/hyper-v/restore-vmsnapshot).
    pub fn restore_vm_snapshot(
        &self,
        vm_name: &str,
        name: &str,
    ) -> VmResult<()> {
        unsafe {
            self.restore_vm_snapshot_unescaped(&escape_pwsh(vm_name), name)
        }
    }

    /// Restores a VM checkpoint named `name`.
    ///
    /// For more information, See [Restore-VMSnapshot](https://docs.microsoft.com/ja-jp/powershell/module/hyper-v/restore-vmsnapshot).
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vm_name` string, which can lead to command injection.
    ///
    /// Please be sure to escape `vm_name` before calling this function.
    ///
    /// `name` will be escaped in this function.
    pub unsafe fn restore_vm_snapshot_unescaped(
        &self,
        vm_name: &str,
        name: &str,
    ) -> VmResult<()> {
        self.cmd("Restore-VMSnapshot")
            .args(&[
                "-VMName",
                vm_name,
                "-Confirm:$false",
                "-Name",
                &escape_pwsh(name),
            ])
            .exec()?;
        Ok(())
    }

    /// Removes a VM checkpoint named `name` from VMs.
    ///
    /// For more information, See [Remove-VMSnapshot](https://docs.microsoft.com/ja-jp/powershell/module/hyper-v/remove-vmsnapshot).
    pub fn remove_vm_snapshot<I>(&self, vms: I, name: &str) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        unsafe {
            self.remove_vm_snapshot_unescaped(
                vms.into_iter().map(escape_pwsh),
                name,
            )
        }
    }

    /// Removes a VM checkpoint named `name` from VMs.
    ///
    /// For more information, See [Remove-VMSnapshot](https://docs.microsoft.com/en-us/powershell/module/hyper-v/remove-vmsnapshot)
    ///
    /// # Safety
    ///
    /// This function doesn't escape `vms` strings, which can lead to command injection.
    ///
    /// Please be sure to escape `vms` before calling this function.
    ///
    /// `name` will be escaped in this function.
    pub unsafe fn remove_vm_snapshot_unescaped<I>(
        &self,
        vms: I,
        name: &str,
    ) -> VmResult<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str> + AsRef<OsStr>,
    {
        self.cmd("Remove-VMSnapshot")
            .arg_array_unescaped(vms)
            .args(&["-Confirm:$false", "-Name", &escape_pwsh(name)])
            .exec()?;
        Ok(())
    }
}

impl VmCmd for HyperVCmd {
    fn list_vms(&self) -> VmResult<Vec<Vm>> { self.get_vm() }

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
        unsafe { self.start_vm_unescaped(&[self.retrieve_vm()?]) }
    }

    fn stop<D: Into<Option<Duration>>>(&self, _timeout: D) -> VmResult<()> {
        unsafe { self.stop_vm_unescaped(&[self.retrieve_vm()?], false, false) }
    }

    fn hard_stop(&self) -> VmResult<()> {
        unsafe { self.stop_vm_unescaped(&[self.retrieve_vm()?], true, false) }
    }

    fn suspend(&self) -> VmResult<()> {
        unsafe { self.suspend_vm_unescaped(&[self.retrieve_vm()?]) }
    }
    fn resume(&self) -> VmResult<()> {
        unsafe { self.resume_vm_unescaped(&[self.retrieve_vm()?]) }
    }

    fn is_running(&self) -> VmResult<bool> {
        Ok(self.get_power_state()? == VmPowerState::Running)
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
        self.get_vm_snapshot()
    }

    fn take_snapshot(&self, name: &str) -> VmResult<()> {
        unsafe { self.checkpoint_vm_unescaped(&[self.retrieve_vm()?], name) }
    }

    fn revert_snapshot(&self, name: &str) -> VmResult<()> {
        unsafe { self.restore_vm_snapshot_unescaped(self.retrieve_vm()?, name) }
    }

    fn delete_snapshot(&self, name: &str) -> VmResult<()> {
        // Remove-VMSnapshot does not change the response regardless of whether a snapshot exists or not.
        let sn = self.list_snapshots()?;
        if !sn.iter().any(|x| x.name.as_deref() == Some(name)) {
            // The snapshot named `name` doesn't exist.
            return vmerr!(ErrorKind::SnapshotNotFound);
        }
        unsafe {
            self.remove_vm_snapshot_unescaped(&[self.retrieve_vm()?], name)
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
            self.copy_vm_file_unescaped(
                &[self.retrieve_vm()?],
                from_guest_path,
                to_host_path,
                true,
                true,
            )
        }
    }

    fn copy_from_host_to_guest(
        &self,
        from_host_path: &str,
        to_guest_path: &str,
    ) -> VmResult<()> {
        unsafe {
            self.copy_vm_file_unescaped(
                &[self.retrieve_vm()?],
                from_host_path,
                to_guest_path,
                true,
                false,
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
