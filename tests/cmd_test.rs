use hvctrl::types::{PowerCmd, ErrorKind, VMResult, VMError, SnapshotCmd, Snapshot};

fn err(r: ErrorKind) -> VMResult<()> { Err(VMError::from(r)) }

/// At first, make sure that a VM is not running.
pub fn power_test(cmd: &impl PowerCmd) {
    assert_eq!(Ok(false), cmd.is_running());
    assert_eq!(Ok(()), cmd.start());
    assert_eq!(err(ErrorKind::VMIsRunning), cmd.start());
    assert_eq!(Ok(true), cmd.is_running());
    assert_eq!(Ok(()), cmd.stop());
    assert_eq!(Ok(false), cmd.is_running());
    assert_eq!(Ok(()), cmd.resume());
    assert_eq!(Ok(true), cmd.is_running());
    assert_eq!(Ok(()), cmd.reboot());
    assert_eq!(Ok(true), cmd.is_running());
    assert_eq!(Ok(()), cmd.hard_reboot());
    assert_eq!(Ok(true), cmd.is_running());
    assert_eq!(Ok(()), cmd.hard_stop());
}

// At first, make sure that the snapshot named `my_snapshot_test` does not exists.
pub fn snapshot_test<T: SnapshotCmd + PowerCmd>(cmd: &T) {
    const SN_NAME: &str = "my_snapshot_test";
    fn is_snapshot_exists(v: &Vec<Snapshot>, name: &str) -> bool {
        v.iter().any(|x| {
            if let Some(n) = &x.name {
                n == name
            } else { false }
        })
    }
    // assert_eq!(Ok(()), cmd.start());
    let v = cmd.list_snapshots();
    assert!(v.is_ok());
    let v = v.unwrap();
    assert!(!is_snapshot_exists(&v, SN_NAME));
    assert_eq!(err(ErrorKind::SnapshotNotFound), cmd.delete_snapshot(SN_NAME));
    assert_eq!(err(ErrorKind::SnapshotNotFound), cmd.revert_snapshot(SN_NAME));
    assert_eq!(Ok(()), cmd.take_snapshot(SN_NAME));
    assert_eq!(Ok(()), cmd.revert_snapshot(SN_NAME));
    assert_eq!(Ok(()), cmd.delete_snapshot(SN_NAME));
    assert_eq!(err(ErrorKind::SnapshotNotFound), cmd.delete_snapshot(SN_NAME));
    assert_eq!(err(ErrorKind::SnapshotNotFound), cmd.revert_snapshot(SN_NAME));
}
