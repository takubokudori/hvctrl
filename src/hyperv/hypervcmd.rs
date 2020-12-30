// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
//! Hyper-V cmdlets controller.
use crate::types::*;
use std::process::Command;

/// PowerShell escape.
///
/// surrounds a command with double quotes and escapes double quotes and back-quotes.
fn pwsh_escape(s: &str) -> String {
    let mut ret = String::with_capacity(s.as_bytes().len() + 2);
    ret.push('"');
    for ch in s.chars() {
        if ch == '`' || ch == '"' { ret.push('`'); }
        ret.push(ch);
    }
    ret.push('"');
    ret
}

/// Represents Hyper-V powershell command executor.
#[derive(Clone, Debug)]
pub struct HyperVCmd {
    executable_path: String,
    vm: String,
}

macro_rules! make_pwsh_array {
    ($cmd:ident, $v:ident)=> (
        $cmd.arg("@(");
        $cmd.args($v.iter().map(|x| pwsh_escape(*x)));
        $cmd.arg(")");
    )
}

impl Default for HyperVCmd {
    fn default() -> Self {
        Self {
            executable_path: "powershell".to_string(),
            vm: "".to_string(),
        }
    }
}

impl HyperVCmd {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the path to HyperVCmd.
    pub fn executable_path<T: Into<String>>(mut self, path: T) -> Self {
        self.executable_path = path.into().trim().to_string();
        self
    }

    pub fn vm<T: Into<String>>(mut self, vm: T) -> Self {
        self.vm = pwsh_escape(&vm.into());
        self
    }

    #[inline]
    fn handle_error(s: &str) -> VMError {
        const IP: &str = "Cannot validate argument on parameter '";
        starts_err!(s, "You do not have the required permission to complete this task.", ErrorKind::PrivilegesRequired);
        starts_err!(s, "Hyper-V was unable to find a virtual machine with name", ErrorKind::VMNotFound);
        starts_err!(s, "The operation cannot be performed while the virtual machine is in its current state.", ErrorKind::InvalidVMState);
        if let Some(s) = s.strip_prefix(IP) {
            let p = s.find("'.").unwrap();
            return VMError::from(ErrorKind::InvalidParameter(s[IP.len()..IP.len() + p].to_string()));
        }
        VMError::from(Repr::Unknown(format!("Unknown error: {}", s)))
    }

    #[inline]
    fn check(s: String, cmd_name: &str) -> VMResult<String> {
        let error_str = format!("{} : ", cmd_name);
        if let Some(s) = s.strip_prefix(&error_str) {
            Err(Self::handle_error(s.trim()))
        } else { Ok(s) }
    }

    fn exec(&self, cmd: &mut Command, cmd_name: &str) -> VMResult<String> {
        let (stdout, stderr) = exec_cmd(cmd)?;
        if !stderr.is_empty() {
            Self::check(stderr, cmd_name)
        } else {
            Ok(stdout)
        }
    }


    #[inline]
    fn cmd(&self) -> Command {
        let mut ret = Command::new(&self.executable_path);
        ret.args(&[
            "-NoProfile",
            "-NoLogo",
            "[Threading.Thread]::CurrentThread.CurrentUICulture = 'en-US';", // Make the output message English.
        ]);
        ret
    }

    pub fn list_vms(&self) -> VMResult<Vec<VM>> {
        let s = self.exec(self.cmd().args(&["Get-VM", "|select Name"]), "Get-VM")?;
        Ok(s.lines()
            .skip(3) // skip `Name\n----`.
            .filter(|x| *x != "")
            .map(|x| {
                let t = x.trim_end().to_string();
                VM {
                    id: Some(t.clone()),
                    name: Some(t),
                    path: None,
                }
            }).collect())
    }

    pub fn start_vm(&self, vms: &[&str]) -> VMResult<()> {
        let mut cmd = self.cmd();
        cmd.arg("Start-VM");
        make_pwsh_array!(cmd, vms);
        self.exec(&mut cmd, "Start-VM")?;
        Ok(())
    }

    pub fn restart_vm(&self, vms: &[&str]) -> VMResult<()> {
        let mut cmd = self.cmd();
        cmd.arg("Restart-VM");
        make_pwsh_array!(cmd, vms);
        self.exec(&mut cmd, "Restart-VM")?;
        Ok(())
    }

    pub fn stop_vm(&self, vms: &[&str], turn_off: bool, use_save: bool) -> VMResult<()> {
        let mut cmd = self.cmd();
        cmd.arg("Stop-VM");
        make_pwsh_array!(cmd, vms);
        self.exec(&mut cmd, "Stop-VM")?;
        if turn_off { cmd.arg("-TurnOff"); }
        if use_save { cmd.arg("-Save"); }
        Ok(())
    }

    pub fn suspend_vm(&self, vms: &[&str]) -> VMResult<()> {
        let mut cmd = self.cmd();
        cmd.arg("Suspend-VM");
        make_pwsh_array!(cmd, vms);
        self.exec(&mut cmd, "Suspend-VM")?;
        Ok(())
    }

    pub fn resume_vm(&self, vms: &[&str]) -> VMResult<()> {
        let mut cmd = self.cmd();
        cmd.arg("Resume-VM");
        make_pwsh_array!(cmd, vms);
        self.exec(&mut cmd, "Resume-VM")?;
        Ok(())
    }

    /// `Copy-VMFile` to copy a file from a guest to host.
    pub fn copy_from_host_to_guest(&self, from_host_path: &str, to_guest_path: &str, create_full_path: bool, is_force: bool) -> VMResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&[
            "Copy-VMFile", self.vm.as_str(),
            "-SourcePath", &pwsh_escape(from_host_path),
            "-DestinationPath", &pwsh_escape(to_guest_path),
            "-FileSource", "Host",
        ]);
        if create_full_path { cmd.arg("-CreateFullPath"); }
        if is_force { cmd.arg("-Force"); }
        let _ = self.exec(&mut cmd, "Copy-VMFile")?;
        Ok(())
    }

    /// `Copy-VMFile` to copy a file from guest to host.
    pub fn copy_from_guest_to_host(&self, from_guest_path: &str, to_host_path: &str, create_full_path: bool, is_force: bool) -> VMResult<()> {
        let mut cmd = self.cmd();
        cmd.args(&[
            "Copy-VMFile", self.vm.as_str(),
            "-SourcePath", &pwsh_escape(from_guest_path),
            "-DestinationPath", &pwsh_escape(to_host_path),
            "-FileSource", "Guest",
        ]);
        if create_full_path { cmd.arg("-CreateFullPath"); }
        if is_force { cmd.arg("-Force"); }
        let _ = self.exec(&mut cmd, "Copy-VMFile")?;
        Ok(())
    }
}

impl PowerCmd for HyperVCmd {
    fn start(&self) -> VMResult<()> {
        self.start_vm(&[&self.vm])
    }

    fn stop(&self) -> VMResult<()> {
        self.stop_vm(&[&self.vm], false, false)
    }

    fn hard_stop(&self) -> VMResult<()> {
        self.stop_vm(&[&self.vm], true, false)
    }

    fn suspend(&self) -> VMResult<()> {
        self.suspend_vm(&[&self.vm])
    }

    fn resume(&self) -> VMResult<()> {
        self.resume_vm(&[&self.vm])
    }

    fn is_running(&self) -> VMResult<bool> {
        unimplemented!()
    }

    fn reboot(&self) -> VMResult<()> {
        self.stop()?;
        self.start()
    }

    fn hard_reboot(&self) -> VMResult<()> {
        self.restart_vm(&[&self.vm])
    }

    fn pause(&self) -> VMResult<()> {
        self.suspend()
    }

    fn unpause(&self) -> VMResult<()> {
        self.resume()
    }
}

#[test]
fn test_pwsh_escape() {
    assert_eq!("\"MSEdge - Win10\"", pwsh_escape("MSEdge - Win10"));
    assert_eq!("\"`\"MSEdge - Win10`\"\"", pwsh_escape("\"MSEdge - Win10\""));
    assert_eq!("\"MSEdge - Win10`\";calc.exe #\"", pwsh_escape("MSEdge - Win10\";calc.exe #"));
    assert_eq!("\"MSEdge - Win10``\"", pwsh_escape("MSEdge - Win10`"));
}
