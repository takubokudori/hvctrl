// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
//! VMware controllers.
#[cfg(any(feature = "vmware", feature = "vmrest"))]
pub mod vmrest;
#[cfg(any(feature = "vmware", feature = "vmrun"))]
pub mod vmrun;

use crate::types::Vm;
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader},
};
#[cfg(any(feature = "vmware", feature = "vmrest"))]
pub use vmrest::*;
#[cfg(any(feature = "vmware", feature = "vmrun"))]
pub use vmrun::*;

/// Gets all VMs from preferences.ini.
///
/// Due to the specification of vmrun, the vmrun command cannot get all VMs.
/// So we need to parse preferences.ini to get all VMs.
#[allow(dead_code)]
pub(crate) fn read_vmware_preferences(
    file_path: &str,
) -> std::io::Result<Option<Vec<Vm>>> {
    let f = std::fs::File::open(file_path)?;
    Ok(parse_preferences(BufReader::new(f)))
}

#[allow(dead_code)]
fn parse_preferences<R: BufRead>(mut f: R) -> Option<Vec<Vm>> {
    fn get_key_value(s: &str) -> Option<(&str, &str)> {
        let kv: Vec<&str> = s.splitn(2, '=').collect();
        if kv.len() < 2 {
            return None;
        }
        let (key, mut value) = (kv[0].trim(), kv[1].trim());
        if value.starts_with('"') && value.ends_with('"') {
            value = &value[1..value.len() - 1];
        }
        Some((key, value))
    }
    let mut s = String::new();
    if f.read_line(&mut s).is_err() {
        return None;
    }
    let enc = get_key_value(&s).and_then(|(key, value)| {
        if key != ".encoding" {
            return None;
        }
        Some(value)
    })?;
    let enc = encoding_rs::Encoding::for_label(enc.as_bytes())?;
    let mut buf = vec![];
    f.read_to_end(&mut buf).ok()?;
    let (s, _, had_error) = enc.decode(&buf);
    if had_error {
        return None;
    }
    let mut vm_list: BTreeMap<u32, Vm> = Default::default();
    for l in s.lines() {
        let kv = get_key_value(l);
        if kv.is_none() {
            continue;
        }
        let (key, value) = kv.unwrap();
        let key_names: Vec<&str> = key.split('.').collect();
        if key_names.len() != 3 {
            continue;
        }
        if let (true, Some(vm_num)) =
            (key_names[0] == "pref", key_names[1].strip_prefix("mruVM"))
        {
            let n: Option<u32> = vm_num.parse().ok();
            if n.is_none() {
                continue;
            }
            let n = n.unwrap();
            vm_list.entry(n).or_insert_with(Vm::default);
            let vm = vm_list.get_mut(&n).unwrap();
            match key_names[2] {
                "filename" => vm.path = Some(value.to_string()),
                "displayName" => vm.name = Some(value.to_string()),
                _ => { /* Does nothing */ }
            }
        }
    }
    Some(vm_list.values().cloned().collect())
}

#[test]
fn test_parse_preferences() {
    let s=r#".encoding = "UTF-8"
pref.keyboardAndMouse.vmHotKey.enabled = "FALSE"
pref.keyboardAndMouse.vmHotKey.count = "0"
pref.ws.session.window.count = "1"
pref.mruVM0.filename = "C:\Users\user\Virtual Machines\CentOS 8 (64 ビット)\CentOS 8 (64 ビット).vmx"
pref.mruVM0.displayName = "CentOS 8 (64 ビット)"
pref.mruVM0.index = "0"
pref.vmplayer.deviceBarToplevel = "FALSE"
vmWizard.isoLocationMRU1.location = "C:\vmware\ubuntu-ja-20.04.1-desktop-amd64.iso"
pref.mruVM1.filename = "C:\Users\user\Documents\Virtual Machines\Ubuntu 64 ビット\Ubuntu 64 ビット.vmx"
pref.mruVM1.displayName = "Ubuntu2004"
pref.mruVM1.index = "1"
"#.as_bytes();
    let s = BufReader::new(s);
    let vm = parse_preferences(s).unwrap();
    assert_eq!(
        vm[0].path.as_deref().unwrap(),
        r"C:\Users\user\Virtual Machines\CentOS 8 (64 ビット)\CentOS 8 (64 ビット).vmx",
    );
    assert_eq!(vm[0].name.as_deref().unwrap(), r"CentOS 8 (64 ビット)");
    assert_eq!(
        vm[1].path.as_deref().unwrap(),
        r"C:\Users\user\Documents\Virtual Machines\Ubuntu 64 ビット\Ubuntu 64 ビット.vmx",
    );
    assert_eq!(vm[1].name.as_deref().unwrap(), r"Ubuntu2004");
    let s=r#".encoding = "UTF-8"
pref.mruVM1.filename = "C:\Users\user\Documents\Virtual Machines\Ubuntu 64 ビット\Ubuntu 64 ビット.vmx"
"#.as_bytes();
    let vm = parse_preferences(s).unwrap();
    assert_eq!(
        vm[0].path.as_deref().unwrap(),
        r"C:\Users\user\Documents\Virtual Machines\Ubuntu 64 ビット\Ubuntu 64 ビット.vmx"
    );
    let s=r#"encoding = "UTF-8"
pref.mruVM1.filename = "C:\Users\user\Documents\Virtual Machines\Ubuntu 64 ビット\Ubuntu 64 ビット.vmx"
"#.as_bytes();
    assert_eq!(parse_preferences(s), None);
    let s=r#".encoding = "Shift_JIS"
pref.mruVM1.filename = "C:\Users\user\Documents\Virtual Machines\Ubuntu 64 ビット\Ubuntu 64 ビット.vmx"
"#.as_bytes();
    assert_eq!(parse_preferences(s), None);
}
