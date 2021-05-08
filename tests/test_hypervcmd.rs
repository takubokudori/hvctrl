// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
//! If you want to run tests, please write your VM configuration to `tests/config.toml`.
//!
//! # config.toml example
//!
//! ```toml
//! [hypervcmd]
//! vm_name = "MyVM"
//! ```

mod test_cmd_util;
#[cfg(test)]
mod tests {
    use crate::test_cmd_util;
    use hvctrl::hyperv::HyperVCmd;
    use serde::Deserialize;
    use hvctrl::types::SnapshotCmd;

    #[derive(Debug, Deserialize)]
    struct HyperVCmdConfig {
        executable_path: Option<String>,
        vm_name: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct ConfigToml {
        hypervcmd: Option<HyperVCmdConfig>,
    }

    fn get_cmd() -> HyperVCmd {
        let x = std::fs::read_to_string("tests/config.toml")
            .expect("Failed to read config.toml");
        let config: ConfigToml =
            toml::from_str(&x).expect("Failed to parse config.toml");
        let config = config
            .hypervcmd
            .as_ref()
            .expect("The configuration of HyperVCmd doesn't exist");
        let mut cmd = HyperVCmd::new();
        if let Some(x) = &config.executable_path {
            cmd.executable_path(x);
        }
        cmd.vm_name(config.vm_name.as_ref().map(|x| x.clone()));
        cmd
    }

    #[test]
    fn test(){
        let cmd=get_cmd();
        cmd.list_vms().unwrap();
        cmd.list_snapshots().unwrap();
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
