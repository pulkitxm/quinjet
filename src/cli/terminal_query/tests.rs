use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;

use super::*;

const MAX_RECORD_BYTES: usize = 64;

#[test]
fn bounded_wait_returns_successful_and_unsuccessful_exit_statuses() {
    for code in [0, 7] {
        let deadline = Instant::now() + Duration::from_millis(500);
        let mut child = Command::new("/bin/sh")
            .args(["-c", &format!("exit {code}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let status = wait_for_child(&mut child, deadline)
            .unwrap()
            .expect("helper exited before its deadline");
        assert_eq!(status.code(), Some(code));
        assert_eq!(status.success(), code == 0);
        assert_eq!(child.try_wait().unwrap(), Some(status));
    }
}

#[test]
fn timed_out_synthetic_executable_is_killed_and_reaped_before_returning() {
    let fixture = tempfile::tempdir().unwrap();
    let executable = fixture.path().join("stalled-helper");
    fs::write(&executable, "#!/bin/sh\nexec /bin/sleep 10\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    for budget in [Duration::ZERO, Duration::from_millis(30)] {
        let started = Instant::now();
        let deadline = started + budget;
        let mut child = Command::new(&executable)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let status = wait_for_child(&mut child, deadline).unwrap();
        assert!(status.is_none());
        assert!(started.elapsed() < Duration::from_secs(2));
        let probe = Command::new("/bin/kill")
            .args(["-0", &child.id().to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(!probe.success(), "helper was reaped before returning");
        let status = child.try_wait().unwrap().expect("helper was reaped");
        assert_eq!(status.signal(), Some(9));
    }
}

#[test]
fn valid_record_is_rejected_until_the_helper_exits() {
    let deadline = Instant::now() + Duration::from_millis(30);
    let mut child = Command::new("/bin/sh")
        .args(["-c", "printf 'kitty 9 18\\n' >&2; exec /bin/sleep 10"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let record = finish_query(&mut child, deadline, MAX_RECORD_BYTES).unwrap();
    assert!(record.is_none());
    let status = child.try_wait().unwrap().expect("helper was reaped");
    assert_eq!(status.signal(), Some(9));
}

#[test]
fn exited_helpers_accept_only_successful_bounded_records() {
    let max_record = [b'0'; MAX_RECORD_BYTES];
    for (script, expected) in [
        (
            "printf 'kitty 9 18\\n' >&2",
            Some(b"kitty 9 18\n".as_slice()),
        ),
        (
            "printf 'sixel 9 18\\n' >&2",
            Some(b"sixel 9 18\n".as_slice()),
        ),
        ("printf 'kitty 9 18\\n' >&2; exit 1", None),
        ("printf '%064d' 0 >&2", Some(max_record.as_slice())),
        ("printf '%065d' 0 >&2", None),
    ] {
        let deadline = Instant::now() + Duration::from_millis(500);
        let mut child = Command::new("/bin/sh")
            .args(["-c", script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let record = finish_query(&mut child, deadline, MAX_RECORD_BYTES).unwrap();
        assert_eq!(record.as_deref(), expected, "{script}");
        assert!(child.try_wait().unwrap().is_some(), "{script}");
    }
}
