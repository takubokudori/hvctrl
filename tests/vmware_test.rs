// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
//! If you want to run tests, please write your VM configuration to `tests/config.toml`.
//!
//! # config.toml example
//!
//! ```toml
//! vmrest_path = "C:\\Program Files (x86)\\VMware\\VMware Player\\vmrest.exe"
//! vmrest_vm = "path\\to\\the\\vm.vmx"
//! vmrest_url = "http://127.0.0.1:8697"
//! vmrest_username = "user"
//! vmrest_password = "password"
//! ```
#[cfg(test)]
mod tests {
    use hvctrl::types::{NICType, PowerCmd};
    use hvctrl::vmware::VMRest;
    use serde::{Serialize, Deserialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct VMRestConfig {
        vmrest_path: Option<String>,
        vmrest_vm: Option<String>,
        vmrest_url: Option<String>,
        vmrest_username: Option<String>,
        vmrest_password: Option<String>,
        vmrest_proxy: Option<String>,
    }


    fn get_cmd() -> VMRest {
        let x = std::fs::read_to_string("tests/config.toml").expect("Failed to read config.toml");
        let config: Result<VMRestConfig, toml::de::Error> = toml::from_str(&x);
        match config {
            Ok(config) => {
                let mut cmd = VMRest::new();
                if let Some(x) = config.vmrest_path { cmd = cmd.vmrest_path(x); }
                if let Some(x) = config.vmrest_url { cmd = cmd.url(x); }
                if let Some(x) = config.vmrest_username { cmd = cmd.username(x); }
                if let Some(x) = config.vmrest_password { cmd = cmd.password(x); }
                if let Some(x) = config.vmrest_proxy { cmd = cmd.proxy(x); }
                if let Some(x) = config.vmrest_vm {
                    let id = cmd.get_vm_id_from_path(&x).expect("VM Not Found");
                    cmd = cmd.vm_id(id);
                }
                cmd
            }
            Err(e) => panic!("Filed to parse config.toml: {}", e),
        }
    }

    #[test]
    fn delete_vms_test() {
        println!("{:?}", get_cmd().delete_vm());
    }

    #[test]
    fn get_ip_address_test() {
        println!("{:?}", get_cmd().get_ip_address());
    }

    #[test]
    fn stop_test() {
        println!("{:?}", get_cmd().stop());
    }

    #[test]
    fn list_vms_test() {
        println!("{:?}", get_cmd().list_vms());
    }

    #[test]
    fn get_power_state_test() {
        println!("{:?}", get_cmd().get_power_state());
    }

    #[test]
    fn list_nics_test() {
        println!("{:?}", get_cmd().list_nics());
    }

    #[test]
    fn list_shared_folders_test() {
        println!("{:?}", get_cmd().list_shared_folders());
    }

    #[test]
    fn mount_shared_folder_test() {
        println!("{:?}", get_cmd().mount_shared_folder("foo", "bar", false));
    }

    #[test]
    fn update_nic_test() {
        println!("{:?}", get_cmd().update_nic(999, &NICType::Bridge));
        println!("{:?}", get_cmd().update_nic(1, &NICType::HostOnly));
    }
}
