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

impl HyperVCmd {
    pub fn new() -> Self {
        Self {
            executable_path: "powershell".to_string(),
            vm: "".to_string(),
        }
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
        if s.starts_with(IP) {
            let p = s[IP.len()..].find("'.").unwrap();
            return VMError::from(ErrorKind::InvalidParameter(s[IP.len()..IP.len() + p].to_string()));
        }
        VMError::from(Repr::Unknown(format!("Unknown error: {}", s)))
    }

    #[inline]
    fn check(s: String, cmd_name: &str) -> VMResult<String> {
        let error_str = format!("{} : ", cmd_name);
        if s.starts_with(&error_str) {
            Err(Self::handle_error(&s[error_str.len()..].trim()))
        } else {
            Ok(s)
        }
    }

    fn exec(&self, cmd: &mut Command, cmd_name: &str) -> VMResult<String> {
        let (stdout, stderr) = exec_cmd(cmd)?;
        if stderr.len() != 0 {
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

    pub fn copy_from_host_to_guest(&self, from_host_path: &str, to_guest_path: &str) -> VMResult<()> {
        let _ = self.exec(self.cmd().args(
            &["Copy-VMFile", self.vm.as_str(),
                "-SourcePath", &pwsh_escape(from_host_path),
                "-DestinationPath", &pwsh_escape(to_guest_path),
                "-CreateFullPath",
                "-FileSource", "Host",
            ]),
                          "Copy-VMFile")?;
        Ok(())
    }

    pub fn copy_from_guest_to_host(&self, from_guest_path: &str, to_host_path: &str) -> VMResult<()> {
        let _ = self.exec(self.cmd().args(
            &["Copy-VMFile", self.vm.as_str(),
                "-SourcePath", &pwsh_escape(from_guest_path),
                "-DestinationPath", &pwsh_escape(to_host_path),
                "-CreateFullPath",
                "-FileSource", "Guest",
            ]),
                          "Copy-VMFile")?;
        Ok(())
    }
}

#[test]
fn test_pwsh_escape() {
    assert_eq!("\"MSEdge - Win10\"", pwsh_escape("MSEdge - Win10"));
    assert_eq!("\"`\"MSEdge - Win10`\"\"", pwsh_escape("\"MSEdge - Win10\""));
    assert_eq!("\"MSEdge - Win10`\";calc.exe #\"", pwsh_escape("MSEdge - Win10\";calc.exe #"));
    assert_eq!("\"MSEdge - Win10``\"", pwsh_escape("MSEdge - Win10`"));
}
