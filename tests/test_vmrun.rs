// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
//! If you want to run tests, please write your VM configuration to `tests/config.toml`.
//!
//! # config.toml example
//!
//! ```toml
//! [vmrun]
//! executable_path = "C:\\Program Files (x86)\\VMware\\VMware Player\\vmrun.exe"
//! host_type = "ws"
//! vm_name = "MyVM"
//! guest_username = "user"
//! guest_password = "password"
//! ```

mod test_cmd_util;

#[cfg(test)]
mod test_vmrun {
    use crate::test_cmd_util;
    use hvctrl::vmware::VmRun;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct VmRunConfig {
        executable_path: Option<String>,
        host_type: Option<String>,
        vm_path: Option<String>,
        guest_username: Option<String>,
        guest_password: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct ConfigToml {
        vmrun: Option<VmRunConfig>,
    }

    fn get_cmd() -> VmRun {
        let x = std::fs::read_to_string("tests/config.toml")
            .expect("Failed to read config.toml");
        let config: ConfigToml =
            toml::from_str(&x).expect("Failed to parse config.toml");
        let mut cmd = VmRun::new();
        let config = config
            .vmrun
            .as_ref()
            .expect("The configuration of VBoxManage doesn't exist");
        if let Some(x) = &config.executable_path {
            cmd.executable_path(x);
        }
        if let Some(x) = &config.host_type {
            cmd.host_type(x);
        }
        cmd.vm_path(config.vm_path.as_ref().map(|x| x.clone()))
            .guest_username(config.guest_username.as_ref().map(|x| x.clone()))
            .guest_password(config.guest_password.as_ref().map(|x| x.clone()))
            .gui(true);
        cmd
    }

    #[test]
    fn test() {
        let cmd = get_cmd();
        regex::Regex::new(r#"^[0-9]-[0-9]+-[0-9]+ build-[0-9]+$"#)
            .unwrap()
            .is_match(&cmd.version().unwrap());
        cmd.list_all_vms().unwrap();
        cmd.list_running_vms().unwrap();
    }

    #[test]
    fn test_vm_cmd() {
        let mut cmd = get_cmd();
        test_cmd_util::test_vm(&mut cmd);
    }

    #[test]
    fn test_power_cmd() {
        let cmd = get_cmd();
        test_cmd_util::test_power(&cmd);
    }

    #[test]
    fn test_snapshot_cmd() {
        let cmd = get_cmd();
        test_cmd_util::test_snapshot_cmd(&cmd);
    }
}
