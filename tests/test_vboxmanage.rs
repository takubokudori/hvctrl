// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
//! If you want to run tests, please write your VM configuration to `tests/config.toml`.
//!
//! # config.toml example
//!
//! ```toml
//! [vboxmanage]
//! executable_path = "C:\\Program Files\\Oracle\\VirtualBox\\VBoxManage.exe"
//! vm_name = "MyVM"
//! guest_username = "user"
//! guest_password = "password"
//! ```

mod test_cmd_util;

#[cfg(test)]
mod tests {
    use crate::test_cmd_util;
    use hvctrl::virtualbox::VBoxManage;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct VBoxManageConfig {
        executable_path: Option<String>,
        vm_name: Option<String>,
        guest_username: Option<String>,
        guest_password: Option<String>,
        guest_domain: Option<String>,
        guest_password_file: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct ConfigToml {
        vboxmanage: Option<VBoxManageConfig>,
    }

    fn get_cmd() -> VBoxManage {
        let x = std::fs::read_to_string("tests/config.toml")
            .expect("Failed to read config.toml");
        let config: ConfigToml =
            toml::from_str(&x).expect("Failed to parse config.toml");
        let mut cmd = VBoxManage::new();
        let config = config
            .vboxmanage
            .as_ref()
            .expect("The configuration of VBoxManage doesn't exist");
        if let Some(x) = &config.executable_path {
            cmd.executable_path(x);
        }
        cmd.vm_name(config.vm_name.as_ref().map(|x| x.clone()))
            .guest_username(config.guest_username.as_ref().map(|x| x.clone()))
            .guest_password(config.guest_password.as_ref().map(|x| x.clone()))
            .guest_domain(config.guest_domain.as_ref().map(|x| x.clone()))
            .guest_password_file(
                config.guest_password_file.as_ref().map(|x| x.clone()),
            );
        cmd
    }

    #[test]
    fn test() {
        let cmd = get_cmd();
        cmd.version().unwrap();
        cmd.list_vms().unwrap();
        cmd.list_snapshots().unwrap();
        cmd.show_vm_info().unwrap();
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
