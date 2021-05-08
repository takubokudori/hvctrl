# HvCtrl

A hypervisor controller library

# Supported OS

Windows only.

# Supported hypervisor controller

- [VirtualBox](https://www.virtualbox.org/)
    - [VBoxManage](https://www.virtualbox.org/manual/ch08.html)
- [VMWare Workstation Player](https://www.vmware.com/products/workstation-player.html)
    - [VMRest](https://code.vmware.com/apis/413)
- [Hyper-V](https://docs.microsoft.com/en-us/virtualization/hyper-v-on-windows/about/)
    - [Hyper-V cmdlets](https://docs.microsoft.com/en-us/powershell/module/hyper-v/?view=win10-ps)

# Example

Write the following lines to Cargo.toml.

```
[dependencies]
hvctrl = {version = "0.1.0", features = ["vboxmanage"]}
```

# License

This software is released under the MIT or Apache-2.0 License, see LICENSE-MIT or LICENSE-APACHE.
