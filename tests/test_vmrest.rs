// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
//! If you want to run tests, please write your VM configuration to `tests/config.toml`.
//!
//! # config.toml example
//!
//! ```toml
//! [vmrest]
//! executable_path = "C:\\Program Files (x86)\\VMware\\VMware Player\\vmrest.exe"
//! vm_path = "path\\to\\the\\vm.vmx"
//! url = "http://127.0.0.1:8697"
//! username = "user"
//! password = "password"
//! ```

mod test_cmd_util;

#[cfg(test)]
mod tests {
    use crate::test_cmd_util;
    use hvctrl::{types::VmCmd, vmware::VmRest};
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct VMRestConfig {
        executable_path: Option<String>,
        vm_path: Option<String>,
        url: Option<String>,
        username: Option<String>,
        password: Option<String>,
        proxy: Option<String>,
        encoding: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct ConfigToml {
        vmrest: Option<VMRestConfig>,
    }

    fn get_cmd() -> VmRest {
        let x = std::fs::read_to_string("tests/config.toml")
            .expect("Failed to read config.toml");
        let config: ConfigToml =
            toml::from_str(&x).expect("Failed to parse config.toml");
        let mut cmd = VmRest::new();
        let config = config
            .vmrest
            .as_ref()
            .expect("The configuration of VMRest doesn't exist");
        if let Some(x) = &config.executable_path {
            cmd.vmrest_path(x);
        }
        if let Some(x) = &config.url {
            cmd.url(x);
        }
        if let Some(x) = &config.encoding {
            cmd.encoding(x);
        }
        cmd.proxy(config.proxy.as_ref().map(|x| x.clone()))
            .username(config.username.as_ref().map(|x| x.clone()))
            .password(config.password.as_ref().map(|x| x.clone()));
        if let Some(x) = &config.vm_path {
            cmd.set_vm_by_path(&x).expect("VM Not Found");
        }
        cmd
    }

    #[test]
    fn test() {
        let cmd = get_cmd();
        println!("version: {:?}", cmd.version().unwrap());
        cmd.get_vms().unwrap();
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
}
