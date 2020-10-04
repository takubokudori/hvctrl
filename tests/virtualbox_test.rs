//! If you want to run tests, please write your VM configuration to `tests/config.toml`.
//!
//! # config.toml example
//!
//! ```toml
//! vboxmanage_path = "C:\\Program Files\\Oracle\\VirtualBox\\VBoxManage.exe"
//! vboxmanage_vm = "MyVM"
//! vboxmanage_guest_username = "user"
//! vboxmanage_guest_password = "password"
//! ```

mod cmd_test;

#[cfg(test)]
mod tests {
    use crate::cmd_test;
    use hvctrl::types::*;
    use hvctrl::virtualbox::VBoxManage;
    use serde::{Serialize, Deserialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct VBoxManageConfig {
        vboxmanage_path: Option<String>,
        vboxmanage_vm: Option<String>,
        vboxmanage_guest_username: Option<String>,
        vboxmanage_guest_password: Option<String>,
        vboxmanage_encoding: Option<String>,
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
                if let Some(x) = config.vboxmanage_guest_username { cmd = cmd.guest_username(x); }
                if let Some(x) = config.vboxmanage_guest_password { cmd = cmd.guest_password(x); }
                if let Some(x) = config.vboxmanage_encoding { cmd = cmd.encoding(&x); }
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
    fn show_vm_info_test() {
        println!("{:?}", get_cmd().show_vm_info());
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
    fn resume_test() {
        println!("{:?}", get_cmd().resume());
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

    #[test]
    fn take_sn_test() {
        println!("{:?}", get_cmd().take_snapshot("sn_test", None, true));
    }

    #[test]
    fn delete_sn() {
        println!("{:?}", get_cmd().take_snapshot("sn_test", None, true));
        println!("{:?}", get_cmd().delete_snapshot("sn_test"));
    }

    #[test]
    fn list_sn(){
        println!("{:?}",get_cmd().list_snapshots());
    }

    #[test]
    fn hard_reboot_a_test() {
        let cmd = get_cmd();
        println!("{:?}", cmd.hard_stop());
        println!("{:?}", cmd.show_vm_info());
        println!("{:?}", cmd.start());
        println!("{:?}", cmd.start());
        println!("{:?}", cmd.start());
        println!("{:?}", cmd.start());
        println!("{:?}", cmd.start());
        println!("{:?}", cmd.start());
        println!("{:?}", cmd.start());
        println!("{:?}", cmd.start());
        println!("{:?}", cmd.start());
        println!("{:?}", cmd.start());
        println!("{:?}", cmd.start());
        println!("{:?}", cmd.start());
    }

    #[test]
    fn power_test() {
        let cmd = get_cmd();
        cmd_test::power_test(&cmd);
    }

    #[test]
    fn snapshot_test() {
        let cmd = get_cmd();
        cmd_test::snapshot_test(&cmd);
    }
}
