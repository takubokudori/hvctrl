# HvCtrl

[![crates.io](https://img.shields.io/crates/v/hvctrl.svg)](https://crates.io/crates/hvctrl)
[![docs.rs](https://docs.rs/hvctrl/badge.svg)](https://docs.rs/hvctrl)

A hypervisor controller library written in Rust.

# Supported OS

Windows only.

# Supported hypervisor controller

- [VirtualBox](https://www.virtualbox.org/)
    - [VBoxManage](https://www.virtualbox.org/manual/ch08.html)
- [VMware Workstation](https://www.vmware.com/products/workstation-player.html)
    - [vmrun](https://docs.vmware.com/en/VMware-Fusion/12/com.vmware.fusion.using.doc/GUID-24F54E24-EFB0-4E94-8A07-2AD791F0E497.html)
    - [VMRest](https://code.vmware.com/apis/413)
- [Hyper-V](https://docs.microsoft.com/en-us/virtualization/hyper-v-on-windows/about/)
    - [Hyper-V cmdlets](https://docs.microsoft.com/en-us/powershell/module/hyper-v/?view=win10-ps)

# Installation

Features that can be used:

- virtualbox
    - vboxmanage
- vmware
    - vmrun
    - vmrest
- hyperv
    - hypervcmd

For example, if you want to control Virtual Box, write the following lines to Cargo.toml.

```
[dependencies]
hvctrl = {git = "0.1.0", features = ["vboxmanage"]}
```

# Examples

See the `examples` directory.

# License

MIT or Apache-2.0 License.
