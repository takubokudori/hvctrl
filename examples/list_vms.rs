//! List all VMs.

#[cfg(feature = "vboxmanage")]
fn list_vbox_vms(exec_path: Option<&String>) {
    use hvctrl::virtualbox::VBoxManage;
    let mut cmd = VBoxManage::new();
    if let Some(e) = exec_path {
        cmd.executable_path(e);
    }
    println!("VBoxManage version: {:?}", cmd.version());
    cmd.list_vms()
        .expect("Failed to list VMs")
        .iter()
        .for_each(|vm| {
            println!("ID: {:?}", vm.id);
            println!("name: {:?}", vm.name);
            println!("path: {:?}", vm.path);
            println!("--------------------");
        })
}

#[cfg(feature = "vmrun")]
fn list_vmware_vms(exec_path: Option<&String>) {
    use hvctrl::vmware::vmrun::VmRun;
    let mut cmd = VmRun::new();
    if let Some(e) = exec_path {
        cmd.executable_path(e);
    }
    println!("VmRun version: {:?}", cmd.version());
    cmd.list_vms()
        .expect("Failed to list VMs")
        .iter()
        .for_each(|vm| {
            println!("ID: {:?}", vm.id);
            println!("name: {:?}", vm.name);
            println!("path: {:?}", vm.path);
            println!("--------------------");
        })
}

#[cfg(feature = "hypervcmd")]
fn list_hyperv_vms(exec_path: Option<&String>) {
    use hvctrl::hyperv::HyperVCmd;
    let mut cmd = HyperVCmd::new();
    if let Some(e) = exec_path {
        cmd.executable_path(e);
    }
    cmd.list_vms()
        .expect("Failed to list VMs")
        .iter()
        .for_each(|vm| {
            println!("ID: {:?}", vm.id);
            println!("name: {:?}", vm.name);
            println!("path: {:?}", vm.path);
            println!("--------------------");
        })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let exec_path = args.get(1);
    #[cfg(feature = "vboxmanage")]
    list_vbox_vms(exec_path);
    #[cfg(feature = "vmrun")]
    list_vmware_vms(exec_path);
    #[cfg(feature = "hypervcmd")]
    list_hyperv_vms(exec_path);
}
