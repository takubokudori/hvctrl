use crate::types::*;
use reqwest::StatusCode;
use serde::{Serialize, Deserialize};
use std::io::Write;
use std::process::Command;

pub struct VMPlayerRest {
    vmrest_path: String,
    url: String,
    vm_id: String,
    proxy: Option<String>,
    encoding: String,
    username: Option<String>,
    password: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum VMPowerState {
    Running,
    Stop,
    Suspend,
    Pause,
    Unknown,
}

#[derive(Debug, Eq, PartialEq)]
pub enum VMRestPowerCommand {
    On,
    Off,
    Shutdown,
    Suspend,
}

impl ToString for VMRestPowerCommand {
    fn to_string(&self) -> String {
        match self {
            Self::On => "on".to_string(),
            Self::Off => "off".to_string(),
            Self::Shutdown => "shutdown".to_string(),
            Self::Suspend => "suspend".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct VMRestFailedResponse {
    #[serde(alias = "Code")]
    code: i32,
    #[serde(alias = "Message")]
    message: String,
}

impl VMPlayerRest {
    pub fn new() -> Self {
        Self {
            vmrest_path: "vmrest".to_string(),
            url: "http://127.0.0.1:8697".to_string(),
            vm_id: "".to_string(),
            proxy: None,
            encoding: "utf-8".to_string(),
            username: None,
            password: None,
        }
    }

    pub fn vmrest_path<T: Into<String>>(mut self, vmrest_path: T) -> Self {
        self.vmrest_path = vmrest_path.into();
        self
    }

    pub fn url<T: Into<String>>(mut self, url: T) -> Self {
        self.url = url.into();
        self
    }

    pub fn vm_id<T: Into<String>>(mut self, vm: T) -> Self {
        self.vm_id = vm.into();
        self
    }

    pub fn username<T: Into<String>>(mut self, username: T) -> Self {
        self.username = Some(username.into());
        self
    }

    pub fn password<T: Into<String>>(mut self, password: T) -> Self {
        self.password = Some(password.into());
        self
    }

    pub fn proxy<T: Into<String>>(mut self, proxy: T) -> Self {
        self.proxy = Some(proxy.into());
        self
    }

    pub fn encoding<T: Into<String>>(mut self, encoding: T) -> Self {
        self.encoding = encoding.into();
        self
    }

    /// Starts vmrest server.
    pub fn start_vmrest_server(&mut self) -> VMResult<()> {
        match Command::new(&self.vmrest_path).output() {
            Ok(x) => {
                let stdout = match String::from_utf8(x.stdout) {
                    Ok(s) => s,
                    Err(x) => return Err(VMError::from(Repr::Unknown(format!("Failed to convert stdout: {}", x.to_string())))),
                };
                for d in stdout.lines() {
                    if d.starts_with("Serving HTTP on ") {
                        self.url = d[16..].to_string();
                        return Ok(());
                    }
                }
                Err(VMError::from(Repr::Unknown("Failed to start a server".to_string())))
            }
            Err(x) => Err(VMError::from(ErrorKind::ExecutionFailed(x.to_string()))),
        }
    }

    /// Creates a vmrest API server account using `vmrest -C`.
    pub fn setup_user(&self, username: &str, password: &str) -> VMResult<()> {
        match Command::new(&self.vmrest_path)
            .arg("-C")
            .spawn() {
            Ok(mut x) => {
                let stdin = x.stdin.as_mut().unwrap();
                stdin.write_fmt(format_args!("{}\n{}\n{}\n", username, password, password)).unwrap();
                match x.wait_with_output() {
                    Ok(_) => Ok(()),
                    Err(x) => Err(VMError::from(ErrorKind::ExecutionFailed(x.to_string()))),
                }
            }
            Err(x) => Err(VMError::from(ErrorKind::ExecutionFailed(x.to_string()))),
        }
    }

    fn execute(&self, v: reqwest::blocking::RequestBuilder) -> VMResult<String> {
        let v = v.header("Accept", "application/vnd.vmware.vmw.rest-v1+json");
        let v = if let Some(x) = &self.username {
            v.basic_auth(x, self.password.as_ref())
        } else { v };
        match v.send() {
            Ok(x) => Self::handle_response(x, &self.encoding),
            Err(x) => Err(VMError::from(ErrorKind::ExecutionFailed(x.to_string()))),
        }
    }

    pub fn get_client(&self) -> VMResult<reqwest::blocking::Client> {
        match self.proxy {
            Some(ref x) => Ok(reqwest::blocking::Client::builder().proxy(reqwest::Proxy::http(x).unwrap()).build().unwrap()),
            None => Ok(reqwest::blocking::Client::new()),
        }
    }

    fn handle_response(resp: reqwest::blocking::Response, encoding: &str) -> VMResult<String> {
        let is_success = resp.status() == StatusCode::OK;
        let text = match resp.text_with_charset(encoding) {
            Ok(x) => x,
            Err(x) => return Err(VMError::from(Repr::Unknown(format!("Failed to convert error: {}", x.to_string())))),
        };
        if is_success {
            Ok(text)
        } else {
            Self::handle_error(text)
        }
    }

    pub fn handle_error(s: String) -> VMResult<String> {
        let ts = s.trim();
        if ts == "404 page not found" {
            return Err(VMError::from(ErrorKind::UnsupportedCommand));
        }
        match serde_json::from_str::<VMRestFailedResponse>(&ts) {
            Ok(x) => Err(Self::handle_json_error(&x.message)),
            Err(_) => Ok(s),
        }
    }

    fn handle_json_error(s: &str) -> VMError {
        match s {
            "Authentication failed" => VMError::from(ErrorKind::AuthenticationFailed),
            "The virtual machine is not powered on" => VMError::from(ErrorKind::VMIsNotPoweredOn),
            _ => VMError::from(Repr::Unknown(format!("Unknown error: {}", s)))
        }
    }

    fn serialize<T: Serialize>(o: &T) -> VMResult<String> {
        match serde_json::to_string(o) {
            Ok(x) => Ok(x),
            Err(x) => Err(VMError::from(ErrorKind::InvalidParameter(x.to_string()))),
        }
    }

    fn deserialize<'a, T: Deserialize<'a>>(s: &'a str) -> VMResult<T> {
        match serde_json::from_str(s) {
            Ok(x) => Ok(x),
            Err(x) => Err(VMError::from(ErrorKind::UnexpectedResponse(x.to_string()))),
        }
    }

    pub fn get_vm_id_from_path(&self, path: &str) -> VMResult<String> {
        let vms = self.list_vms()?;
        for vm in vms {
            if let Some(p) = vm.path {
                if p == path { return Ok(vm.id.unwrap()); }
            }
        }
        Err(VMError::from(ErrorKind::VMNotFound))
    }

    // (vm name, id)
    pub fn list_vms(&self) -> VMResult<Vec<VM>> {
        let cli = self.get_client()?;
        let v = cli.get(&format!("{}/api/vms", self.url));
        let s = self.execute(v)?;
        Ok(Self::deserialize(&s)?)
    }

    pub fn get_power_state(&self) -> VMResult<VMPowerState> {
        let cli = self.get_client()?;
        let v = cli.get(&format!("{}/api/vms/{}/power", self.url, self.vm_id));
        let s = self.execute(v)?;
        #[derive(Deserialize)]
        struct Resp { power_state: String }
        let r: Resp = Self::deserialize(&s)?;
        match r.power_state.as_str() {
            "poweredOn" => Ok(VMPowerState::Running),
            "poweredOff" => Ok(VMPowerState::Stop),
            "suspended" => Ok(VMPowerState::Suspend),
            x => Err(VMError::from(ErrorKind::UnexpectedResponse(x.to_string()))),
        }
    }

    pub fn set_power_state(&self, state: &VMRestPowerCommand) -> VMResult<()> {
        let cli = self.get_client()?;
        let v = cli.put(&format!("{}/api/vms/{}/power", self.url, self.vm_id));
        let v = v.body(state.to_string());
        let s = self.execute(v)?;
        #[derive(Deserialize)]
        struct Resp { power_state: String }
        let r: Resp = Self::deserialize(&s)?;
        match (r.power_state.as_str(), state) {
            ("poweredOn", VMRestPowerCommand::On)
            | ("poweredOff", VMRestPowerCommand::Off)
            | ("poweredOff", VMRestPowerCommand::Shutdown)
            | ("suspended", VMRestPowerCommand::Suspend) => Ok(()),
            _ => Err(VMError::from(Repr::Unknown("Failed to change power state".to_string()))),
        }
    }

    pub fn get_ip_address(&self) -> VMResult<String> {
        let cli = self.get_client()?;
        let v = cli.get(&format!("{}/api/vms/{}/ip", self.url, self.vm_id));
        let s = self.execute(v)?;
        #[derive(Deserialize)]
        struct Resp { ip: String }
        let r: Resp = Self::deserialize(&s)?;
        Ok(r.ip)
    }

    pub fn list_nics(&self) -> VMResult<Vec<NIC>> {
        fn conv(s: &str) -> NICType {
            match s {
                "bridged" => NICType::Bridge,
                "nat" => NICType::NAT,
                "hostonly" => NICType::HostOnly,
                "custom" => NICType::Custom,
                _ => panic!("Unknown type: {}", s),
            }
        }

        let cli = self.get_client()?;
        let v = cli.get(&format!("{}/api/vms/{}/nic", self.url, self.vm_id));
        let s = self.execute(v)?;
        #[derive(Deserialize)]
        struct NICDevice {
            index: i32,
            #[serde(alias = "type")]
            ty: String,
            vmnet: String,
            #[serde(alias = "macAddress")]
            mac_address: String,
        }

        #[derive(Deserialize)]
        struct NICDevices { num: usize, nics: Vec<NICDevice> }
        let r: NICDevices = Self::deserialize(&s)?;
        assert_eq!(r.num, r.nics.len());
        Ok(r.nics.iter().map(|x| {
            NIC {
                id: Some(x.index.to_string()),
                name: Some(x.vmnet.clone()),
                ty: Some(conv(&x.ty)),
                mac_address: Some(x.mac_address.clone()),
            }
        }).collect())
    }

    pub fn list_shared_folders(&self) -> VMResult<Vec<SharedFolder>> {
        let cli = self.get_client()?;
        let v = cli.get(&format!("{}/api/vms/{}/sharedfolders", self.url, self.vm_id));
        let s = self.execute(v)?;
        #[derive(Deserialize)]
        struct SHF {
            folder_id: String,
            host_path: String,
            /// 0(R) or 4(RW)
            flags: i32,
        }
        let r: Vec<SHF> = Self::deserialize(&s)?;
        Ok(r.iter().map(|x| {
            SharedFolder {
                id: Some(x.folder_id.clone()),
                name: None,
                guest_path: None,
                host_path: Some(x.host_path.clone()),
                is_readonly: x.flags != 4,
            }
        }).collect())
    }

    pub fn mount_shared_folders(&self, shfs: Vec<SharedFolder>) -> VMResult<()> {
        let cli = self.get_client()?;
        #[derive(Serialize)]
        struct SHF {
            folder_id: String,
            host_path: String,
            /// 0(R) or 4(RW)
            flags: i32,
        }
        let v = cli.post(&format!("{}/api/vms/{}/sharedfolders", self.url, self.vm_id))
            .header("Content-Type", "application/vnd.vmware.vmw.rest-v1+json")
            .body(Self::serialize(&shfs.iter().map(|x| {
                SHF {
                    folder_id: x.id.as_ref().unwrap().to_string(),
                    host_path: x.host_path.as_ref().unwrap().to_string(),
                    flags: if x.is_readonly { 0 } else { 4 },
                }
            }).collect::<Vec<SHF>>())?);
        let s = self.execute(v)?;
        println!("ret:{}", s);
        Ok(())
    }

    pub fn mount_shared_folder(&self, folder_id: &str, host_path: &str, is_readonly: bool) -> VMResult<()> {
        self.mount_shared_folders(vec![SharedFolder {
            id: Some(folder_id.to_string()),
            name: None,
            guest_path: None,
            host_path: Some(host_path.to_string()),
            is_readonly,
        }])
    }
}

impl PowerCmd for VMPlayerRest {
    fn start(&self) -> VMResult<()> { self.set_power_state(&VMRestPowerCommand::On) }

    fn stop(&self) -> VMResult<()> { self.set_power_state(&VMRestPowerCommand::Shutdown) }

    fn hard_stop(&self) -> VMResult<()> { self.set_power_state(&VMRestPowerCommand::Off) }

    fn suspend(&self) -> VMResult<()> { self.set_power_state(&VMRestPowerCommand::On) }

    fn resume(&self) -> VMResult<()> { self.set_power_state(&VMRestPowerCommand::On) }

    fn is_running(&self) -> VMResult<bool> { Ok(self.get_power_state()? == VMPowerState::Running) }

    fn reboot(&self) -> VMResult<()> {
        let _ = self.stop();
        self.start()
    }

    fn hard_reboot(&self) -> VMResult<()> {
        let _ = self.hard_stop();
        self.start()
    }

    fn pause(&self) -> VMResult<()> { Err(VMError::from(ErrorKind::UnsupportedCommand)) }

    fn unpause(&self) -> VMResult<()> { Err(VMError::from(ErrorKind::UnsupportedCommand)) }
}
