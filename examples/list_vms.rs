use hvctrl::virtualbox::VBoxManage;
fn main() {
    let cmd = VBoxManage::new();
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
