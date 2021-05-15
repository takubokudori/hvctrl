// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
//! VMRest controller.
use crate::{deserialize, types::*};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    process::Command,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum VmRestPowerCommand {
    On,
    Off,
    Shutdown,
    Suspend,
}

impl VmRestPowerCommand {
    pub fn to_command(&self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
            Self::Shutdown => "shutdown",
            Self::Suspend => "suspend",
        }
    }
}

impl ToString for VmRestPowerCommand {
    fn to_string(&self) -> String { self.to_command().to_string() }
}

#[derive(Deserialize)]
struct NicDevice {
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

impl<T: AsRef<str>> From<T> for NicType {
    fn from(s: T) -> Self {
        match s.as_ref() {
            "bridged" => Self::Bridge,
            "nat" => Self::NAT,
            "hostOnly" => Self::HostOnly,
            "custom" => Self::Custom("".to_string()),
            _ => panic!("Unknown type: {}", s.as_ref()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct VmRest {
    executable_path: String,
    url: String,
    vm_id: Option<String>,
    proxy: Option<String>,
    encoding: String,
    username: Option<String>,
    password: Option<String>,
}

impl Default for VmRest {
    fn default() -> Self { Self::new() }
}

impl VmRest {
    pub fn new() -> Self {
        Self {
            executable_path: "vmrest".to_string(),
            url: "http://127.0.0.1:8697".to_string(),
            encoding: "utf-8".to_string(),
            vm_id: None,
            proxy: None,
            username: None,
            password: None,
        }
    }

    impl_setter!(executable_path: String);

    pub fn url<T: Into<String>>(&mut self, url: T) -> &mut Self {
        self.url = url.into();
        if !self.url.starts_with("http://") && self.url.starts_with("https://")
        {
            panic!("Invalid scheme specified in url: {}", self.url);
        }
        self
    }

    impl_setter!(@opt vm_id: String);
    impl_setter!(@opt username: String);
    impl_setter!(@opt password: String);
    impl_setter!(@opt proxy: String);
    impl_setter!(encoding: String);

    /// Starts vmrest server.
    pub fn start_vmrest_server(&mut self, port: Option<u16>) -> VmResult<()> {
        let mut cmd = Command::new(&self.executable_path);
        if let Some(port) = port {
            cmd.args(&["-p", &port.to_string()]);
        }
        let (stdout, _) = exec_cmd(&mut cmd)?;
        for d in stdout.lines() {
            if let Some(url) = d.strip_prefix("Serving HTTP on ") {
                self.url = format!("http://{}", url);
                return Ok(());
            }
        }
        vmerr!(Repr::Unknown("Failed to start a server".to_string()))
    }

    /// Creates a vmrest API server account using `vmrest -C`.
    pub fn setup_user(&self, username: &str, password: &str) -> VmResult<()> {
        match Command::new(&self.executable_path).arg("-C").spawn() {
            Ok(mut x) => {
                let stdin = x.stdin.as_mut().unwrap();
                stdin
                    .write_fmt(format_args!(
                        "{}\n{}\n{}\n",
                        username, password, password
                    ))
                    .unwrap();
                match x.wait_with_output() {
                    Ok(_) => Ok(()),
                    Err(x) => vmerr!(ErrorKind::ExecutionFailed(x.to_string())),
                }
            }
            Err(x) => vmerr!(ErrorKind::ExecutionFailed(x.to_string())),
        }
    }

    fn execute(
        &self,
        v: reqwest::blocking::RequestBuilder,
    ) -> VmResult<String> {
        let v = v.header("Accept", "application/vnd.vmware.vmw.rest-v1+json");
        let v = if let Some(x) = &self.username {
            v.basic_auth(x, self.password.as_ref())
        } else {
            v
        };
        match v.send() {
            Ok(x) => Self::handle_response(x, &self.encoding),
            Err(x) => vmerr!(ErrorKind::ExecutionFailed(x.to_string())),
        }
    }

    pub fn get_client(&self) -> VmResult<reqwest::blocking::Client> {
        match self.proxy {
            Some(ref x) => Ok(reqwest::blocking::Client::builder()
                .proxy(reqwest::Proxy::http(x).unwrap())
                .build()
                .unwrap()),
            None => Ok(reqwest::blocking::Client::new()),
        }
    }

    fn handle_response(
        resp: reqwest::blocking::Response,
        encoding: &str,
    ) -> VmResult<String> {
        let is_success = resp.status() == StatusCode::OK;
        let text = match resp.text_with_charset(encoding) {
            Ok(x) => x,
            Err(x) => {
                return vmerr!(Repr::Unknown(format!(
                    "Failed to convert error: {}",
                    x.to_string()
                )));
            }
        };
        if is_success {
            Ok(text)
        } else {
            Self::handle_error(text)
        }
    }

    pub fn handle_error(s: String) -> VmResult<String> {
        #[derive(Debug, Clone, Deserialize)]
        struct VmRestFailedResponse {
            #[serde(alias = "Code")]
            code: i32,
            #[serde(alias = "Message")]
            message: String,
        }

        let ts = s.trim();
        if ts == "404 page not found" {
            return vmerr!(ErrorKind::UnsupportedCommand);
        }
        match serde_json::from_str::<VmRestFailedResponse>(&ts) {
            Ok(x) => Err(Self::handle_json_error(&x.message)),
            Err(_) => Ok(s),
        }
    }

    fn handle_json_error(s: &str) -> VmError {
        const RP: &str = "Redundant parameter: ";
        const OOP: &str = "One of the parameters was invalid: ";
        if let Some(s) = s.strip_prefix(RP) {
            return VmError::from(ErrorKind::InvalidParameter(s.to_string()));
        }
        if let Some(s) = s.strip_prefix(OOP) {
            return VmError::from(ErrorKind::InvalidParameter(s.to_string()));
        }
        match s {
            "Authentication failed" => {
                VmError::from(ErrorKind::AuthenticationFailed)
            }
            "The virtual machine is not powered on" => VmError::from(
                ErrorKind::InvalidPowerState(VmPowerState::NotRunning),
            ),
            "The virtual network cannot be found" => {
                VmError::from(ErrorKind::NetworkNotFound)
            }
            "The network adapter cannot be found" => {
                VmError::from(ErrorKind::NetworkAdaptorNotFound)
            }
            _ => VmError::from(Repr::Unknown(format!("Unknown error: {}", s))),
        }
    }

    fn serialize<T: Serialize>(o: &T) -> VmResult<String> {
        match serde_json::to_string(o) {
            Ok(x) => Ok(x),
            Err(x) => vmerr!(ErrorKind::InvalidParameter(x.to_string())),
        }
    }

    /// Gets the VM ID from the path.
    pub fn get_vm_id_by_path(&self, path: &str) -> VmResult<String> {
        let vms = self.get_vms()?;
        for vm in vms {
            if path == vm.path.as_deref().expect("Failed to get path") {
                return Ok(vm.id.expect("Failed to get id"));
            }
        }
        vmerr!(ErrorKind::VmNotFound)
    }

    fn get_vm_id(&self) -> VmResult<&str> {
        self.vm_id
            .as_deref()
            .ok_or_else(|| VmError::from(ErrorKind::VmIsNotSpecified))
    }

    pub fn version(&self) -> VmResult<String> {
        let cli = self.get_client()?;
        let v = cli.get(&format!("{}/json/swagger.json", self.url));
        let s = self.execute(v)?;

        fn find<'a>(s: &'a str, pat: &str) -> VmResult<&'a str> {
            match s.find(pat) {
                Some(x) => Ok(&s[x + pat.len()..]),
                None => vmerr!(ErrorKind::UnexpectedResponse(s.to_string())),
            }
        }
        let s = find(&s, "description\"")?;
        let s = find(s, "\"")?;
        let m = s.find(',').unwrap();
        Ok(s[..m - 1].to_string())
    }

    pub fn get_vms(&self) -> VmResult<Vec<Vm>> {
        let cli = self.get_client()?;
        let v = cli.get(&format!("{}/api/vms", self.url));
        let s = self.execute(v)?;
        deserialize(&s)
    }

    pub fn delete_vm(&self) -> VmResult<()> {
        let cli = self.get_client()?;
        let v =
            cli.delete(&format!("{}/api/vms/{}", self.url, self.get_vm_id()?));
        let s = self.execute(v)?;
        deserialize(&s)
    }

    pub fn get_power_state(&self) -> VmResult<VmPowerState> {
        let cli = self.get_client()?;
        let v = cli.get(&format!(
            "{}/api/vms/{}/power",
            self.url,
            self.get_vm_id()?
        ));
        let s = self.execute(v)?;
        #[derive(Deserialize)]
        struct Resp {
            power_state: String,
        }
        let r: Resp = deserialize(&s)?;
        match r.power_state.as_str() {
            "poweredOn" => Ok(VmPowerState::Running),
            "poweredOff" => Ok(VmPowerState::Stopped),
            "suspended" => Ok(VmPowerState::Suspended),
            x => vmerr!(ErrorKind::UnexpectedResponse(x.to_string())),
        }
    }

    pub fn set_power_state(
        &self,
        state: &VmRestPowerCommand,
    ) -> VmResult<VmPowerState> {
        let cli = self.get_client()?;
        let v = cli
            .put(&format!("{}/api/vms/{}/power", self.url, self.get_vm_id()?))
            .header("Content-Type", "application/vnd.vmware.vmw.rest-v1+json")
            .body(state.to_command());
        let s = self.execute(v)?;
        #[derive(Deserialize)]
        struct Resp {
            power_state: String,
        }
        let r: Resp = deserialize(&s)?;
        match r.power_state.as_str() {
            "poweredOn" => Ok(VmPowerState::Running),
            "poweredOff" => Ok(VmPowerState::Stopped),
            "suspended" => Ok(VmPowerState::Suspended),
            x => {
                vmerr!(ErrorKind::UnexpectedResponse(format!(
                    "set_power_state: {}",
                    x
                )))
            }
        }
    }

    pub fn get_ip_address(&self) -> VmResult<String> {
        let cli = self.get_client()?;
        let v =
            cli.get(&format!("{}/api/vms/{}/ip", self.url, self.get_vm_id()?));
        let s = self.execute(v)?;
        #[derive(Deserialize)]
        struct Resp {
            ip: String,
        }
        let r: Resp = deserialize(&s)?;
        Ok(r.ip)
    }

    pub fn list_nics(&self) -> VmResult<Vec<Nic>> {
        let cli = self.get_client()?;
        let v =
            cli.get(&format!("{}/api/vms/{}/nic", self.url, self.get_vm_id()?));
        let s = self.execute(v)?;

        #[derive(Deserialize)]
        struct NicDevices {
            num: usize,
            nics: Vec<NicDevice>,
        }
        let r: NicDevices = deserialize(&s)?;
        assert_eq!(r.num, r.nics.len());
        Ok(r.nics
            .iter()
            .map(|x| Nic {
                id: Some(x.index.to_string()),
                name: Some(x.vmnet.clone()),
                ty: Some(x.ty.as_str().into()),
                mac_address: Some(x.mac_address.clone()),
            })
            .collect())
    }

    pub fn create_nic(&self, ty: &NicType) -> VmResult<Nic> {
        let cli = self.get_client()?;
        #[derive(Serialize)]
        struct Req {
            #[serde(rename(serialize = "type"))]
            ty: String,
            vmnet: Option<String>,
        }
        let v = cli
            .post(&format!("{}/api/vms/{}/nic", self.url, self.get_vm_id()?))
            .header("Content-Type", "application/vnd.vmware.vmw.rest-v1+json")
            .body(Self::serialize({
                let (ty, vmnet) = match ty {
                    NicType::NAT => ("nat".to_string(), None),
                    NicType::Bridge => ("bridged".to_string(), None),
                    NicType::HostOnly => ("hostonly".to_string(), None),
                    NicType::Custom(x) => {
                        ("custom".to_string(), Some(x.to_string()))
                    }
                };
                &Req { ty, vmnet }
            })?);

        let s = self.execute(v)?;
        let r: NicDevice = deserialize(&s)?;

        Ok(Nic {
            id: Some(r.index.to_string()),
            name: Some(r.vmnet),
            ty: Some(r.ty.into()),
            mac_address: Some(r.mac_address),
        })
    }

    pub fn update_nic(&self, index: i32, ty: &NicType) -> VmResult<()> {
        let cli = self.get_client()?;
        #[derive(Serialize)]
        struct Req {
            #[serde(rename(serialize = "type"))]
            ty: String,
            vmnet: Option<String>,
        }
        let v = cli
            .put(&format!(
                "{}/api/vms/{}/nic/{}",
                self.url,
                self.get_vm_id()?,
                index
            ))
            .header("Content-Type", "application/vnd.vmware.vmw.rest-v1+json")
            .body(Self::serialize({
                let (ty, vmnet) = match ty {
                    NicType::NAT => ("nat".to_string(), None),
                    NicType::Bridge => ("bridged".to_string(), None),
                    NicType::HostOnly => ("hostonly".to_string(), None),
                    NicType::Custom(x) => {
                        ("custom".to_string(), Some(x.to_string()))
                    }
                };
                &Req { ty, vmnet }
            })?);

        let s = self.execute(v)?;
        let r: NicDevice = deserialize(&s)?;
        if r.index != index {
            return vmerr!(ErrorKind::UnexpectedResponse(format!(
                "{}",
                r.index
            )));
        }
        Ok(())
    }

    pub fn delete_nic(&self, index: i32) -> VmResult<()> {
        let cli = self.get_client()?;
        let v = cli.delete(&format!(
            "{}/api/vms/{}/nic/{}",
            self.url,
            self.get_vm_id()?,
            index
        ));
        self.execute(v)?;
        Ok(())
    }

    pub fn list_shared_folders(&self) -> VmResult<Vec<SharedFolder>> {
        let cli = self.get_client()?;
        let v = cli.get(&format!(
            "{}/api/vms/{}/sharedfolders",
            self.url,
            self.get_vm_id()?
        ));
        let s = self.execute(v)?;
        #[derive(Deserialize)]
        struct Resp {
            folder_id: String,
            host_path: String,
            /// 0(R) or 4(RW)
            flags: i32,
        }
        let r: Vec<Resp> = deserialize(&s)?;
        Ok(r.iter()
            .map(|x| SharedFolder {
                id: Some(x.folder_id.clone()),
                name: None,
                guest_path: None,
                host_path: Some(x.host_path.clone()),
                is_readonly: x.flags != 4,
            })
            .collect())
    }

    pub fn mount_shared_folders(&self, shfs: &[&SharedFolder]) -> VmResult<()> {
        let cli = self.get_client()?;
        #[derive(Serialize)]
        struct ShfReq {
            folder_id: String,
            host_path: String,
            /// 0(R) or 4(RW)
            flags: i32,
        }
        let v = cli
            .post(&format!(
                "{}/api/vms/{}/sharedfolders",
                self.url,
                self.get_vm_id()?
            ))
            .header("Content-Type", "application/vnd.vmware.vmw.rest-v1+json")
            .body(Self::serialize(
                &shfs
                    .iter()
                    .map(|x| ShfReq {
                        folder_id: x.id.as_ref().unwrap().to_string(),
                        host_path: x.host_path.as_ref().unwrap().to_string(),
                        flags: if x.is_readonly { 0 } else { 4 },
                    })
                    .collect::<Vec<ShfReq>>(),
            )?);
        let _ = self.execute(v)?;
        Ok(())
    }

    pub fn mount_shared_folder(
        &self,
        folder_id: &str,
        host_path: &str,
        is_readonly: bool,
    ) -> VmResult<()> {
        self.mount_shared_folders(&[&SharedFolder {
            id: Some(folder_id.to_string()),
            name: None,
            guest_path: None,
            host_path: Some(host_path.to_string()),
            is_readonly,
        }])
    }

    pub fn delete_shared_folder(&self, folder_id: &str) -> VmResult<()> {
        let cli = self.get_client()?;
        let v = cli.delete(&format!(
            "{}/api/vms/{}/sharedfolders/{}",
            self.url,
            self.get_vm_id()?,
            folder_id
        ));
        self.execute(v)?;
        Ok(())
    }

    pub fn get_display_name(&self) -> VmResult<String> {
        self.get_display_name_by_id(self.get_vm_id()?)
    }

    pub fn get_display_name_by_id(&self, id: &str) -> VmResult<String> {
        for vm in self.get_vms()? {
            if id == vm.id.as_deref().expect("Failed to get id") {
                let path = vm.path.as_deref().unwrap();
                return Self::get_display_name_from_vmx(path)
                    .ok_or_else(|| VmError::from(ErrorKind::VmNotFound));
            }
        }
        vmerr!(ErrorKind::VmNotFound)
    }

    fn get_display_name_from_vmx(path: &str) -> Option<String> {
        use std::io::{BufRead, BufReader};
        // Return `None` if the vmx file cannot be opened.
        if let Ok(f) = std::fs::File::open(path) {
            for l in BufReader::new(f).lines().flatten() {
                if let Some(dn) = l.strip_prefix("displayName = \"") {
                    if dn.is_empty() {
                        // broken?
                        return None;
                    }
                    let dn = &dn[..dn.len() - 1];
                    return Some(dn.to_string());
                }
            }
        }
        None
    }

    fn is_running_result(&self) -> VmResult<()> {
        if !self.get_power_state()?.is_running() {
            vmerr!(ErrorKind::InvalidPowerState(VmPowerState::NotRunning))
        } else {
            Ok(())
        }
    }
}

fn expected_power_state(
    res: VmResult<VmPowerState>,
    expected: VmPowerState,
) -> VmResult<()> {
    match res {
        Ok(x) if x == expected => Ok(()),
        Ok(x) => vmerr!(ErrorKind::InvalidPowerState(x)),
        Err(x) => Err(x),
    }
}

impl VmCmd for VmRest {
    fn list_vms(&self) -> VmResult<Vec<Vm>> { self.get_vms() }

    fn set_vm_by_id(&mut self, id: &str) -> VmResult<()> {
        for vm in self.get_vms()? {
            if id == vm.id.as_deref().expect("Failed to get id") {
                self.vm_id = vm.id;
                return Ok(());
            }
        }
        vmerr!(ErrorKind::VmNotFound)
    }

    /// `name` is the name of a VM as displayed in the GUI, not the `.vmx` file name.
    fn set_vm_by_name(&mut self, name: &str) -> VmResult<()> {
        for vm in self.get_vms()? {
            let path = vm.path.as_deref().unwrap();
            // Ignore if the vmx file cannot be opened.
            if let Some(display_name) = Self::get_display_name_from_vmx(path) {
                if name == display_name {
                    self.vm_id = vm.id;
                    return Ok(());
                }
            }
        }
        vmerr!(ErrorKind::VmNotFound)
    }

    fn set_vm_by_path(&mut self, path: &str) -> VmResult<()> {
        self.vm_id = Some(self.get_vm_id_by_path(path)?);
        Ok(())
    }
}

impl PowerCmd for VmRest {
    fn start(&self) -> VmResult<()> {
        if self.get_power_state()?.is_running() {
            return vmerr!(ErrorKind::InvalidPowerState(VmPowerState::Running));
        }
        expected_power_state(
            self.set_power_state(&VmRestPowerCommand::On),
            VmPowerState::Running,
        )
    }

    fn stop<D: Into<Option<Duration>>>(&self, timeout: D) -> VmResult<()> {
        let timeout = timeout.into();
        let s = Instant::now();
        self.is_running_result()?;
        loop {
            match self.set_power_state(&VmRestPowerCommand::Shutdown) {
                Ok(VmPowerState::Stopped) => return Ok(()),
                Ok(VmPowerState::Running) => { /* Does nothing */ }
                Ok(x) => return vmerr!(ErrorKind::InvalidPowerState(x)),
                Err(x) => return Err(x),
            }

            if let Some(timeout) = timeout {
                if s.elapsed() >= timeout {
                    return vmerr!(ErrorKind::Timeout);
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    fn hard_stop(&self) -> VmResult<()> {
        self.is_running_result()?;
        expected_power_state(
            self.set_power_state(&VmRestPowerCommand::Off),
            VmPowerState::Stopped,
        )
    }

    fn suspend(&self) -> VmResult<()> {
        self.is_running_result()?;
        expected_power_state(
            self.set_power_state(&VmRestPowerCommand::Suspend),
            VmPowerState::Suspended,
        )
    }

    fn resume(&self) -> VmResult<()> { self.start() }

    fn is_running(&self) -> VmResult<bool> {
        Ok(self.get_power_state()? == VmPowerState::Running)
    }

    fn reboot<D: Into<Option<Duration>>>(&self, timeout: D) -> VmResult<()> {
        self.is_running_result()?;
        self.stop(timeout)?;
        self.start()
    }

    fn hard_reboot(&self) -> VmResult<()> {
        self.is_running_result()?;
        let _ = self.hard_stop();
        self.start()
    }

    fn pause(&self) -> VmResult<()> { vmerr!(ErrorKind::UnsupportedCommand) }

    fn unpause(&self) -> VmResult<()> { vmerr!(ErrorKind::UnsupportedCommand) }
}

impl NicCmd for VmRest {
    fn list_nics(&self) -> VmResult<Vec<Nic>> { VmRest::list_nics(self) }

    fn add_nic(&self, nic: &Nic) -> VmResult<()> {
        if let Some(ty) = &nic.ty {
            VmRest::create_nic(self, ty)?;
        } else {
            return vmerr!(ErrorKind::InvalidParameter(
                "ty is required".to_string()
            ));
        }
        Ok(())
    }

    fn update_nic(&self, nic: &Nic) -> VmResult<()> {
        if let (Some(index), Some(ty)) = (&nic.id, &nic.ty) {
            VmRest::update_nic(self, index.parse().unwrap_or(0), ty)
        } else {
            vmerr!(ErrorKind::InvalidParameter(
                "id and ty are required".to_string()
            ))
        }
    }

    fn remove_nic(&self, nic: &Nic) -> VmResult<()> {
        if let Some(index) = &nic.id {
            self.delete_nic(index.parse().unwrap_or(0))
        } else {
            vmerr!(ErrorKind::InvalidParameter("id is required".to_string()))
        }
    }
}

impl SharedFolderCmd for VmRest {
    fn list_shared_folders(&self) -> VmResult<Vec<SharedFolder>> {
        VmRest::list_shared_folders(self)
    }

    fn mount_shared_folder(&self, shfs: &SharedFolder) -> VmResult<()> {
        VmRest::mount_shared_folders(self, &[shfs])
    }

    fn unmount_shared_folder(&self, shfs: &SharedFolder) -> VmResult<()> {
        SharedFolderCmd::delete_shared_folder(self, shfs)
    }

    fn delete_shared_folder(&self, shfs: &SharedFolder) -> VmResult<()> {
        if let Some(id) = &shfs.id {
            Self::delete_shared_folder(self, id)
        } else {
            vmerr!(ErrorKind::InvalidParameter("id is required".to_string()))
        }
    }
}
