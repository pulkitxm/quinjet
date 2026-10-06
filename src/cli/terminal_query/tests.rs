use std::os::unix::process::ExitStatusExt;

use super::*;

const MAX_RECORD_BYTES: usize = 64;

#[test]
fn timed_out_helper_is_killed_and_reaped_before_returning() {
    let mut child = Command::new("/bin/sleep")
        .arg("10")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let started = Instant::now();
    let record = finish_helper(
        &mut child,
        started + Duration::from_millis(30),
        MAX_RECORD_BYTES,
    )
    .unwrap();
    assert!(record.is_none());
    assert!(started.elapsed() < Duration::from_secs(2));
    let status = child.try_wait().unwrap().expect("helper was reaped");
    assert_eq!(status.signal(), Some(9));
}

#[test]
fn valid_record_is_rejected_until_the_helper_exits() {
    let mut child = Command::new("/bin/sh")
        .args(["-c", "printf 'kitty 9 18\\n' >&2; exec /bin/sleep 10"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let record = finish_helper(
        &mut child,
        Instant::now() + Duration::from_millis(30),
        MAX_RECORD_BYTES,
    )
    .unwrap();
    assert!(record.is_none());
    let status = child.try_wait().unwrap().expect("helper was reaped");
    assert_eq!(status.signal(), Some(9));
}

#[test]
fn exited_helpers_accept_only_successful_bounded_records() {
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
        ("printf '%065d' 0 >&2", None),
    ] {
        let mut child = Command::new("/bin/sh")
            .args(["-c", script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let record = finish_helper(
            &mut child,
            Instant::now() + Duration::from_millis(500),
            MAX_RECORD_BYTES,
        )
        .unwrap();
        assert_eq!(record.as_deref(), expected, "{script}");
        assert!(child.try_wait().unwrap().is_some(), "{script}");
    }
}
