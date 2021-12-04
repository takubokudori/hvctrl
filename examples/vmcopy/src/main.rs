// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
use clap::{App, Arg, ArgMatches};
use hvctrl::types::{GuestCmd, VmCmd};
use std::{
    convert::{TryFrom, TryInto},
    fmt::Formatter,
    io::Write,
};

#[derive(Copy, Clone)]
enum Tool {
    HyperV,
    VirtualBox,
    VMware,
}

impl std::fmt::Display for Tool {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Tool::HyperV => "Hyper-V",
            Tool::VirtualBox => "VirtualBox",
            Tool::VMware => "VMware",
        }
        .fmt(f)
    }
}

impl TryFrom<&str> for Tool {
    type Error = ();

    fn try_from(x: &str) -> Result<Self, ()> {
        let x = x.to_ascii_lowercase();
        match x.as_ref() {
            "hyper-v" | "hyperv" | "hv" => Ok(Self::HyperV),
            "virtualbox" | "vbox" | "vb" => Ok(Self::VirtualBox),
            "vmware" | "vw" => Ok(Self::VMware),
            _ => Err(()),
        }
    }
}

fn input(m: &ArgMatches, name: &str, disp: &str) -> String {
    match m.value_of(name) {
        Some(x) => x.to_string(),
        None => {
            print!("{}: ", disp);
            std::io::stdout().flush().expect("Failed to flush stdout");
            let mut s = String::new();
            std::io::stdin().read_line(&mut s).unwrap();
            s.lines().next().unwrap().to_string()
        }
    }
}

fn input_password(m: &ArgMatches, name: &str) -> String {
    match m.value_of(name) {
        Some(x) => x.to_string(),
        None => {
            std::io::stdout().flush().expect("Failed to flush stdout");
            let s =
                rpassword::prompt_password_stdout("Guest password: ").unwrap();
            s.lines().next().unwrap().to_string()
        }
    }
}

fn input_vm_name(m: &ArgMatches, name: &str, cmd: &dyn VmCopyCmd) -> String {
    match m.value_of(name) {
        Some(x) => x.to_string(),
        None => {
            let vms: Vec<String> = cmd
                .list_vms()
                .expect("Failed to list VMs")
                .iter()
                .filter(|vm| vm.name.is_some())
                .map(|vm| vm.name.as_ref().unwrap().to_owned())
                .collect();
            input_list("name", &vms)
        }
    }
}

fn input_list<T: AsRef<str>>(header: &str, l: &[T]) -> String {
    println!("\nid: {}", header);
    println!("-------");
    l.iter()
        .enumerate()
        .for_each(|(i, name)| println!("{}: {}", i, name.as_ref()));
    println!("-------");
    print!("id: ");
    std::io::stdout().flush().expect("Failed to flush stdout");
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).unwrap();
    let s = s.trim_end();
    match s.parse::<usize>() {
        Ok(x) => l[x].as_ref().to_string(),
        Err(_) => s.to_string(),
    }
}

trait VmCopyCmd: VmCmd + GuestCmd {
    fn gu(&mut self, gu: Option<String>);
    fn gp(&mut self, gp: Option<String>);
}

impl VmCopyCmd for hvctrl::virtualbox::vboxmanage::VBoxManage {
    fn gu(&mut self, gu: Option<String>) { self.guest_username(gu); }

    fn gp(&mut self, gp: Option<String>) { self.guest_password(gp); }
}

impl VmCopyCmd for hvctrl::hyperv::HyperVCmd {
    fn gu(&mut self, gu: Option<String>) { self.guest_username(gu); }

    fn gp(&mut self, gp: Option<String>) { self.guest_password(gp); }
}
impl VmCopyCmd for hvctrl::vmware::VmRun {
    fn gu(&mut self, gu: Option<String>) { self.guest_username(gu); }

    fn gp(&mut self, gp: Option<String>) { self.guest_password(gp); }
}

fn get_cmd(
    tool: Tool,
    exec_path: Option<&str>,
    use_player: bool,
) -> Box<dyn VmCopyCmd> {
    match tool {
        Tool::HyperV => {
            let mut ret = Box::new(hvctrl::hyperv::HyperVCmd::new());
            if let Some(x) = exec_path {
                ret.executable_path(x);
            }
            ret
        }
        Tool::VirtualBox => {
            let mut ret =
                Box::new(hvctrl::virtualbox::vboxmanage::VBoxManage::new());
            if let Some(x) = exec_path {
                ret.executable_path(x);
            }
            ret
        }
        Tool::VMware => {
            let mut ret = Box::new(hvctrl::vmware::VmRun::new());
            if let Some(x) = exec_path {
                ret.executable_path(x);
            }
            if use_player {
                ret.use_inventory(false);
            }
            ret
        }
    }
}

fn main() {
    env_logger::init();
    let m = App::new("VMCopy")
        .arg(Arg::new("tool").short('t').long("tool").takes_value(true))
        .arg(
            Arg::new("executable_path")
                .short('e')
                .long("exec")
                .takes_value(true)
                .about(
                    "A tool to send a file to VM. The parameter is Hyper-V, \
                     VirtualBox or VMware",
                ),
        )
        .arg(
            Arg::new("use_default_exe")
                .long("use-default-exe")
                .about("Use default executable path"),
        )
        .arg(
            Arg::new("copy_from_guest")
                .long("copy-from-guest")
                .about("Copy a file from guest flag"),
        )
        .arg(
            Arg::new("vm_name")
                .short('n')
                .long("vm")
                .takes_value(true)
                .about("VM name to send a file"),
        )
        .arg(
            Arg::new("src")
                .short('s')
                .long("src")
                .takes_value(true)
                .about("A source path on host"),
        )
        .arg(
            Arg::new("dst")
                .short('d')
                .long("dst")
                .takes_value(true)
                .about("A destination path on guest"),
        )
        .arg(
            Arg::new("guest_username")
                .short('u')
                .long("gu")
                .takes_value(true)
                .about("A guest username at logon"),
        )
        .arg(
            Arg::new("guest_password")
                .short('p')
                .long("gp")
                .takes_value(true)
                .about("A guest password at logon"),
        )
        .arg(
            Arg::new("use_player")
                .long("player")
                .about("use VMware Player"),
        )
        .get_matches();

    let tool = m.value_of("tool").map_or_else(
        || {
            input_list("tool", &["Hyper-V", "VirtualBox", "VMware"])
                .as_str()
                .try_into()
                .unwrap()
        },
        |x| x.try_into().unwrap(),
    );
    println!("\nTool: {}", tool);

    let exec_path = if !m.is_present("use_default_exe") {
        input(&m, "executable_path", "Executable path")
    } else {
        "".to_string()
    };
    let exec_path = if exec_path.is_empty() {
        None
    } else {
        Some(exec_path.as_str())
    };
    let use_player = m.is_present("use_player");
    let copy_from_guest = m.is_present("copy_from_guest");
    let mut cmd = get_cmd(tool, exec_path, use_player);

    let vm_name = input_vm_name(&m, "vm_name", cmd.as_ref());
    match tool {
        Tool::VirtualBox | Tool::VMware => {
            cmd.gu(Some(input(&m, "gu", "Guest username")));
            cmd.gp(Some(input_password(&m, "gp")));
        }
        Tool::HyperV => {
            if copy_from_guest {
                cmd.gu(Some(input(&m, "gu", "Guest username")));
                cmd.gp(Some(input_password(&m, "gp")));
            }
        }
    }

    let src = input(&m, "src", "Source path");
    let dst = input(&m, "dst", "Destination path");

    println!("\nExecutable path: {:?}", exec_path);
    println!(
        "Copy from: {}",
        if copy_from_guest { "Guest" } else { "Host" }
    );
    println!("VM name: {}", vm_name);
    println!("Source path: {}", src);
    println!("Destination path: {}", dst);

    cmd.set_vm_by_name(&vm_name).unwrap();

    {
        print!("\nOK?[Y/N] ");
        std::io::stdout().flush().expect("Failed to flush stdout");
        let mut s = String::new();
        std::io::stdin().read_line(&mut s).unwrap();
        let s = s.trim();
        if s.starts_with('N') || s.starts_with('n') {
            println!("aborted!");
            return;
        }
    }

    if copy_from_guest {
        match cmd.copy_from_guest_to_host(&src, &dst) {
            Ok(_) => println!("success!"),
            Err(x) => println!("Failed to copy a file: {:?}", x),
        }
    } else {
        match cmd.copy_from_host_to_guest(&src, &dst) {
            Ok(_) => println!("success!"),
            Err(x) => println!("Failed to copy a file: {:?}", x),
        }
    }

    print!("Press enter!");
    std::io::stdout().flush().expect("Failed to flush stdout");
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).unwrap();
}
