// Copyright takubokudori.
// This source code is licensed under the MIT or Apache-2.0 license.
#![allow(dead_code)]
use hvctrl::{
    types::{ErrorKind, PowerCmd, Snapshot, SnapshotCmd, VmResult},
    vmerr,
};
use std::time::Duration;

fn is_invalid_state_running<T>(x: VmResult<T>) -> bool {
    match x {
        Ok(_) => panic!("The function succeeded unexpectedly"),
        Err(x) => match x.is_invalid_state_running() {
            Some(x) => x,
            None => panic!("Unexpected error: {}", x.to_string()),
        },
    }
}

fn assert_ok_stop(cmd: &impl PowerCmd, timeout: Duration) {
    let status = cmd.stop(timeout);
    if status == Ok(()) {
        assert_eq!(Ok(false), cmd.is_running());
    } else if status == vmerr!(ErrorKind::Timeout) {
        assert_eq!(Ok(true), cmd.is_running());
    } else if let Err(x) = status {
        panic!("Unexpected error: {}", x.to_string());
    }
}

fn assert_ok_reboot(cmd: &impl PowerCmd, timeout: Duration) {
    let status = cmd.reboot(timeout);
    if status == Ok(()) {
        assert_eq!(Ok(true), cmd.is_running());
    } else if status == vmerr!(ErrorKind::Timeout) {
        assert_eq!(Ok(true), cmd.is_running());
    } else if let Err(x) = status {
        panic!("Unexpected error: {}", x.to_string());
    }
}

/// At first, make sure that a VM is not running.
pub fn test_power(cmd: &impl PowerCmd) {
    let timeout = Duration::from_secs(3);
    assert_eq!(Ok(false), cmd.is_running());
    assert!(!is_invalid_state_running(cmd.hard_stop()));
    assert!(!is_invalid_state_running(cmd.stop(timeout)));
    assert!(!is_invalid_state_running(cmd.hard_reboot()));
    assert!(!is_invalid_state_running(cmd.reboot(timeout)));

    assert_eq!(Ok(()), cmd.start());
    assert!(is_invalid_state_running(cmd.start()));
    assert_eq!(Ok(true), cmd.is_running());
    assert!(is_invalid_state_running(cmd.start()));
    assert_eq!(Ok(()), cmd.suspend());
    assert_eq!(Ok(false), cmd.is_running());
    assert!(!is_invalid_state_running(cmd.suspend()));
    assert_eq!(Ok(()), cmd.resume());
    assert_eq!(Ok(true), cmd.is_running());
    assert!(is_invalid_state_running(cmd.resume()));
    assert_eq!(Ok(()), cmd.hard_stop());
    assert_eq!(Ok(false), cmd.is_running());

    assert_eq!(Ok(()), cmd.start());
    assert_eq!(Ok(true), cmd.is_running());
    assert_ok_reboot(cmd, timeout);
    let _ = cmd.hard_stop();
    assert_eq!(Ok(false), cmd.is_running());
    assert_eq!(Ok(()), cmd.start());
    assert_eq!(Ok(true), cmd.is_running());
    assert_ok_stop(cmd, timeout);
    let _ = cmd.hard_stop();
    assert_eq!(Ok(false), cmd.is_running());
}

/// At first, make sure that the snapshot named `hvctrl_test_snapshot` does not exists.
pub fn test_snapshot_cmd<T: SnapshotCmd>(cmd: &T) {
    const SN_NAME: &str = "hvctrl_test_snapshot";
    fn is_snapshot_exists(v: &[Snapshot], name: &str) -> bool {
        v.iter()
            .any(|x| x.name.as_ref().map_or(false, |n| n == name))
    }
    let v = cmd
        .list_snapshots()
        .expect("Failed to get the list of snapshots");
    // check the snapshot doesn't exist.
    assert!(!is_snapshot_exists(&v, SN_NAME));
    assert_eq!(
        vmerr!(ErrorKind::SnapshotNotFound),
        cmd.delete_snapshot(SN_NAME)
    );
    assert_eq!(
        vmerr!(ErrorKind::SnapshotNotFound),
        cmd.revert_snapshot(SN_NAME)
    );

    // take a snapshot and check `revert` and `delete`.
    assert_eq!(Ok(()), cmd.take_snapshot(SN_NAME));
    assert_eq!(Ok(()), cmd.revert_snapshot(SN_NAME));
    assert_eq!(Ok(()), cmd.delete_snapshot(SN_NAME));
    assert_eq!(
        vmerr!(ErrorKind::SnapshotNotFound),
        cmd.delete_snapshot(SN_NAME)
    );
    assert_eq!(
        vmerr!(ErrorKind::SnapshotNotFound),
        cmd.revert_snapshot(SN_NAME)
    );
}
