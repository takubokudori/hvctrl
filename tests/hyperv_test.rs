// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
//! If you want to run tests, please write your VM configuration to `tests/config.toml`.
//!
//! # config.toml example
//!
//! ```toml
//! hyperv_cmd_vm = "MyVM"
//! ```

#[cfg(test)]
mod tests {
    use hvctrl::hyperv::HyperVCmd;
    use serde::{Serialize, Deserialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct HyperVCmdConfig {
        powershell_path: Option<String>,
        hyperv_cmd_vm: Option<String>,
    }

    fn get_cmd() -> HyperVCmd {
        let x = std::fs::read_to_string("tests/config.toml").expect("Failed to read config.toml");
        let config: Result<HyperVCmdConfig, toml::de::Error> = toml::from_str(&x);
        match config {
            Ok(config) => {
                let mut cmd = HyperVCmd::new();
                if let Some(x) = config.powershell_path { cmd = cmd.executable_path(x); }
                if let Some(x) = config.hyperv_cmd_vm { cmd = cmd.vm(x); }
                cmd
            }
            Err(e) => panic!("Filed to parse config.toml: {}", e),
        }
    }

    #[test]
    fn list_vms_test() {
        println!("{:?}", get_cmd().list_vms());
    }
}
