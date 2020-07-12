//! If you want to run tests, please write your VM configuration to `tests/config.toml`.
//!
//! # config.toml example
//!
//! ```toml
//! path = "C:\\Program Files\\Oracle\\VirtualBox\\VBoxManage.exe"
//! vm = "MyVM"
//! guest_username = "user"
//! guest_password = "password"
//! ```
#[cfg(test)]
mod tests {
    use hvctrl::types::*;
    use hvctrl::virtualbox::VBoxManage;
    use serde::{Serialize, Deserialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct VBoxManageConfig {
        vboxmanage_path: Option<String>,
        vboxmanage_vm: Option<String>,
        guest_username: Option<String>,
        guest_password: Option<String>,
    }

    fn assert_uuid(x: &str) {
        assert!(regex::Regex::new(r#"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$"#).unwrap().is_match(x))
    }

    fn get_cmd() -> VBoxManage {
        let x = std::fs::read_to_string("tests/config.toml").expect("Failed to read config.toml");
        let config: Result<VBoxManageConfig, toml::de::Error> = toml::from_str(&x);
        match config {
            Ok(config) => {
                let mut cmd = VBoxManage::new();
                if let Some(x) = config.vboxmanage_path { cmd = cmd.executable_path(x); }
                if let Some(x) = config.vboxmanage_vm { cmd = cmd.vm(x); }
                if let Some(x) = config.guest_username { cmd = cmd.guest_username(x); }
                if let Some(x) = config.guest_password { cmd = cmd.guest_password(x); }
                cmd
            }
            Err(e) => panic!("Filed to parse config.toml: {}", e),
        }
    }

    #[test]
    fn version_test() {
        println!("{:?}", get_cmd().version());
    }

    #[test]
    fn is_running_test() {
        println!("{:?}", get_cmd().is_running());
    }

    #[test]
    fn list_vms_test() {
        println!("{:?}", get_cmd().list_vms());
    }

    #[test]
    fn start_test() {
        println!("{:?}", get_cmd().start());
    }

    #[test]
    fn list_snapshots_test() {
        let v = get_cmd().list_snapshots();
        match v {
            Ok(v) => {
                v.iter().for_each(|x| assert_uuid(x.id.as_ref().unwrap()));
                println!("{:?}", v);
            }
            x => println!("{:?}", x),
        }
    }

    #[test]
    fn reboot_test() {
        println!("{:?}", get_cmd().reboot());
    }

    #[test]
    fn stop_test() {
        println!("{:?}", get_cmd().stop());
    }

    #[test]
    fn suspend_test() {
        println!("{:?}", get_cmd().suspend());
    }

    #[test]
    fn run_command_test() {
        println!("{:?}", get_cmd().run_command(&["C:\\Windows\\notepad.exe"]));
    }

    #[test]
    fn copy_from_test() {
        println!("{:?}", get_cmd().copy_from("C:\\Windows\\notepad.exe", "C:\\test"));
    }

    #[test]
    fn copy_to_test() {
        println!("{:?}", get_cmd().copy_to("C:\\Windows\\notepad.exe", "C:\\test"));
    }
}
