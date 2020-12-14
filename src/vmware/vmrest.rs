// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
//! VMRest controller.
use crate::types::*;
use reqwest::StatusCode;
use serde::{Serialize, Deserialize};
use std::io::Write;
use std::process::Command;

#[derive(Clone, Debug)]
pub struct VMRest {
    vmrest_path: String,
    url: String,
    vm_id: String,
    proxy: Option<String>,
    encoding: String,
    username: Option<String>,
    password: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum VMRestPowerCommand {
    On,
    Off,
    Shutdown,
    Suspend,
}

#[derive(Deserialize)]
struct NICDevice {
    index: i32,
    #[serde(alias = "type")]
    #[allow(dead_code)]
    ty: String,
    #[allow(dead_code)]
    vmnet: String,
    #[serde(alias = "macAddress")]
    #[allow(dead_code)]
    mac_address: String,
}

fn conv(s: &str) -> NICType {
    match s {
        "bridged" => NICType::Bridge,
        "nat" => NICType::NAT,
        "hostOnly" => NICType::HostOnly,
        "custom" => NICType::Custom("".to_string()),
        _ => panic!("Unknown type: {}", s),
    }
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

impl VMRest {
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
                    Err(x) => return vmerr!(Repr::Unknown(format!("Failed to convert stdout: {}", x.to_string()))),
                };
                for d in stdout.lines() {
                    const SHO: &str = "Serving HTTP on ";
                    if d.starts_with(SHO) {
                        self.url = d[SHO.len()..].to_string();
                        return Ok(());
                    }
                }
                vmerr!(Repr::Unknown("Failed to start a server".to_string()))
            }
            Err(x) => vmerr!(ErrorKind::ExecutionFailed(x.to_string())),
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
                    Err(x) => vmerr!(ErrorKind::ExecutionFailed(x.to_string())),
                }
            }
            Err(x) => vmerr!(ErrorKind::ExecutionFailed(x.to_string())),
        }
    }

    fn execute(&self, v: reqwest::blocking::RequestBuilder) -> VMResult<String> {
        let v = v.header("Accept", "application/vnd.vmware.vmw.rest-v1+json");
        let v = if let Some(x) = &self.username {
            v.basic_auth(x, self.password.as_ref())
        } else { v };
        match v.send() {
            Ok(x) => Self::handle_response(x, &self.encoding),
            Err(x) => vmerr!(ErrorKind::ExecutionFailed(x.to_string())),
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
            Err(x) => return vmerr!(Repr::Unknown(format!("Failed to convert error: {}", x.to_string()))),
        };
        if is_success {
            Ok(text)
        } else {
            Self::handle_error(text)
        }
    }

    pub fn handle_error(s: String) -> VMResult<String> {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct VMRestFailedResponse {
            #[serde(alias = "Code")]
            code: i32,
            #[serde(alias = "Message")]
            message: String,
        }

        let ts = s.trim();
        if ts == "404 page not found" {
            return vmerr!(ErrorKind::UnsupportedCommand);
        }
        match serde_json::from_str::<VMRestFailedResponse>(&ts) {
            Ok(x) => Err(Self::handle_json_error(&x.message)),
            Err(_) => Ok(s),
        }
    }

    fn handle_json_error(s: &str) -> VMError {
        const RP: &str = "Redundant parameter: ";
        const OOP: &str = "One of the parameters was invalid: ";
        starts_err!(s,RP,ErrorKind::InvalidParameter(s[RP.len()..].to_string()));
        starts_err!(s,OOP,ErrorKind::InvalidParameter(s[OOP.len()..].to_string()));

        match s {
            "Authentication failed" => VMError::from(ErrorKind::AuthenticationFailed),
            "The virtual machine is not powered on" => VMError::from(ErrorKind::VMIsNotRunning),
            "The virtual network cannot be found" => VMError::from(ErrorKind::NetworkNotFound),
            "The network adapter cannot be found" => VMError::from(ErrorKind::NetworkAdaptorNotFound),
            _ => VMError::from(Repr::Unknown(format!("Unknown error: {}", s)))
        }
    }

    fn serialize<T: Serialize>(o: &T) -> VMResult<String> {
        match serde_json::to_string(o) {
            Ok(x) => Ok(x),
            Err(x) => vmerr!(ErrorKind::InvalidParameter(x.to_string())),
        }
    }

    fn deserialize<'a, T: Deserialize<'a>>(s: &'a str) -> VMResult<T> {
        match serde_json::from_str(s) {
            Ok(x) => Ok(x),
            Err(x) => vmerr!(ErrorKind::UnexpectedResponse(x.to_string())),
        }
    }

    pub fn get_vm_id_from_path(&self, path: &str) -> VMResult<String> {
        let vms = self.list_vms()?;
        for vm in vms {
            if let Some(p) = vm.path {
                if p == path { return Ok(vm.id.unwrap()); }
            }
        }
        vmerr!(ErrorKind::VMNotFound)
    }

    pub fn list_vms(&self) -> VMResult<Vec<VM>> {
        let cli = self.get_client()?;
        let v = cli.get(&format!("{}/api/vms", self.url));
        let s = self.execute(v)?;
        Ok(Self::deserialize(&s)?)
    }

    pub fn delete_vm(&self) -> VMResult<()> {
        let cli = self.get_client()?;
        let v = cli.delete(&format!("{}/api/vms/{}", self.url, self.vm_id));
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
            "poweredOff" => Ok(VMPowerState::Stopped),
            "suspended" => Ok(VMPowerState::Suspended),
            x => vmerr!(ErrorKind::UnexpectedResponse(x.to_string())),
        }
    }

    pub fn set_power_state(&self, state: &VMRestPowerCommand) -> VMResult<()> {
        let cli = self.get_client()?;
        let v = cli.put(&format!("{}/api/vms/{}/power", self.url, self.vm_id))
            .header("Content-Type", "application/vnd.vmware.vmw.rest-v1+json")
            .body(state.to_string());
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
        let cli = self.get_client()?;
        let v = cli.get(&format!("{}/api/vms/{}/nic", self.url, self.vm_id));
        let s = self.execute(v)?;

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

    pub fn create_nic(&self, ty: &NICType) -> VMResult<NIC> {
        let cli = self.get_client()?;
        #[derive(Serialize)]
        struct Req {
            #[serde(rename(serialize = "type"))]
            ty: String,
            vmnet: Option<String>,
        }
        let v = cli.post(&format!("{}/api/vms/{}/nic", self.url, self.vm_id))
            .header("Content-Type", "application/vnd.vmware.vmw.rest-v1+json")
            .body(Self::serialize({
                let (ty, vmnet) = match ty {
                    NICType::NAT => ("nat".to_string(), None),
                    NICType::Bridge => ("bridged".to_string(), None),
                    NICType::HostOnly => ("hostonly".to_string(), None),
                    NICType::Custom(x) => ("custom".to_string(), Some(x.to_string())),
                };
                &Req {
                    ty,
                    vmnet,
                }
            })?);

        let s = self.execute(v)?;
        let r: NICDevice = Self::deserialize(&s)?;

        Ok(NIC {
            id: Some(r.index.to_string()),
            name: Some(r.vmnet),
            ty: Some(conv(&r.ty)),
            mac_address: Some(r.mac_address),
        })
    }

    pub fn update_nic(&self, index: i32, ty: &NICType) -> VMResult<()> {
        let cli = self.get_client()?;
        #[derive(Serialize)]
        struct Req {
            #[serde(rename(serialize = "type"))]
            ty: String,
            vmnet: Option<String>,
        }
        let v = cli.put(&format!("{}/api/vms/{}/nic/{}", self.url, self.vm_id, index))
            .header("Content-Type", "application/vnd.vmware.vmw.rest-v1+json")
            .body(Self::serialize({
                let (ty, vmnet) = match ty {
                    NICType::NAT => ("nat".to_string(), None),
                    NICType::Bridge => ("bridged".to_string(), None),
                    NICType::HostOnly => ("hostonly".to_string(), None),
                    NICType::Custom(x) => ("custom".to_string(), Some(x.to_string())),
                };
                &Req {
                    ty,
                    vmnet,
                }
            })?);

        let s = self.execute(v)?;
        let r: NICDevice = Self::deserialize(&s)?;
        if r.index != index {
            return vmerr!(ErrorKind::UnexpectedResponse(format!("{}",r.index)));
        }
        Ok(())
    }

    pub fn delete_nic(&self, index: i32) -> VMResult<()> {
        let cli = self.get_client()?;
        let v = cli.delete(&format!("{}/api/vms/{}/nic/{}", self.url, self.vm_id, index));
        self.execute(v)?;
        Ok(())
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

    pub fn mount_shared_folders(&self, shfs: &[&SharedFolder]) -> VMResult<()> {
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
        let _ = self.execute(v)?;
        Ok(())
    }

    pub fn mount_shared_folder(&self, folder_id: &str, host_path: &str, is_readonly: bool) -> VMResult<()> {
        self.mount_shared_folders(&[&SharedFolder {
            id: Some(folder_id.to_string()),
            name: None,
            guest_path: None,
            host_path: Some(host_path.to_string()),
            is_readonly,
        }])
    }

    pub fn delete_shared_folder(&self, folder_id: &str) -> VMResult<()> {
        let cli = self.get_client()?;
        let v = cli.delete(&format!("{}/api/vms/{}/sharedfolders/{}", self.url, self.vm_id, folder_id));
        self.execute(v)?;
        Ok(())
    }
}

impl PowerCmd for VMRest {
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

    fn pause(&self) -> VMResult<()> { vmerr!(ErrorKind::UnsupportedCommand) }

    fn unpause(&self) -> VMResult<()> { vmerr!(ErrorKind::UnsupportedCommand) }
}

impl NICCmd for VMRest {
    fn list_nics(&self) -> VMResult<Vec<NIC>> {
        VMRest::list_nics(self)
    }

    fn add_nic(&self, nic: &NIC) -> VMResult<()> {
        if let Some(ty) = &nic.ty {
            VMRest::create_nic(self, ty)?;
        } else {
            return vmerr!(ErrorKind::InvalidParameter("ty is required".to_string()));
        }
        Ok(())
    }

    fn update_nic(&self, nic: &NIC) -> VMResult<()> {
        if let (Some(index), Some(ty)) = (&nic.id, &nic.ty) {
            VMRest::update_nic(self, index.parse().unwrap_or(0), ty)
        } else {
            vmerr!(ErrorKind::InvalidParameter("id and ty are required".to_string()))
        }
    }

    fn remove_nic(&self, nic: &NIC) -> VMResult<()> {
        if let Some(index) = &nic.id {
            self.delete_nic(index.parse().unwrap_or(0))
        } else {
            vmerr!(ErrorKind::InvalidParameter("id is required".to_string()))
        }
    }
}

impl SharedFolderCmd for VMRest {
    fn list_shared_folders(&self) -> VMResult<Vec<SharedFolder>> {
        VMRest::list_shared_folders(self)
    }

    fn mount_shared_folder(&self, shfs: &SharedFolder) -> VMResult<()> {
        VMRest::mount_shared_folders(self, &[shfs])
    }

    fn unmount_shared_folder(&self, shfs: &SharedFolder) -> VMResult<()> {
        SharedFolderCmd::delete_shared_folder(self, shfs)
    }

    fn delete_shared_folder(&self, shfs: &SharedFolder) -> VMResult<()> {
        if let Some(id) = &shfs.id {
            Self::delete_shared_folder(self, id)
        } else {
            vmerr!(ErrorKind::InvalidParameter("id is required".to_string()))
        }
    }
}
