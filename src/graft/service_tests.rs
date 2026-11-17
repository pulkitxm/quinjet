#![expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]

use std::io::Read as _;
use std::net::TcpListener;

use super::*;

fn paths() -> (tempfile::TempDir, ServicePaths) {
    let directory = tempfile::tempdir().unwrap();
    let paths = ServicePaths {
        root: directory.path().join("service"),
    };
    fs::create_dir_all(&paths.root).unwrap();
    (directory, paths)
}

fn status_server() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let mut buffer = [0_u8; 1024];
            drop(stream.read(&mut buffer));
            drop(stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok"));
        }
    });
    port
}

fn state(port: u16, module: &str) -> ServiceState {
    ServiceState {
        pid: 7,
        port,
        token: "secret".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        module: module.to_owned(),
        gateway: PathBuf::from("/opt/gg"),
        log: PathBuf::from("/tmp/gateway.log"),
        started: 1,
    }
}

#[test]
fn leases_and_calls_keep_the_gateway_alive_until_they_go_quiet() {
    let (_directory, paths) = paths();
    let now = SystemTime::now();
    assert_eq!(paths.live_clients(now), 0);
    let lease = paths.lease().unwrap();
    assert_eq!(paths.live_clients(SystemTime::now()), 1);
    paths.touch_activity().unwrap();
    assert_eq!(paths.live_clients(SystemTime::now()), 2);
    assert_eq!(
        paths.live_clients(SystemTime::now() + Duration::from_secs(120)),
        0
    );
    drop(lease);
    assert_eq!(paths.live_clients(SystemTime::now()), 1);
    let abandoned = paths.leases().join("99999-dead");
    fs::write(&abandoned, b"").unwrap();
    assert_eq!(
        paths.live_clients(SystemTime::now() + Duration::from_secs(120)),
        0
    );
    assert!(!abandoned.exists());
}

#[test]
fn a_current_gateway_is_reused_and_a_missing_one_is_started_once() {
    let (_directory, paths) = paths();
    let port = status_server();
    let mut starts = 0;
    let started = paths
        .ensure(
            "module-a",
            &mut || {
                starts += 1;
                paths.write_state(&state(port, "module-a"))
            },
            Duration::from_secs(5),
        )
        .unwrap();
    assert_eq!(started.port, port);
    assert_eq!(started.mcp_url(), format!("http://127.0.0.1:{port}/mcp"));
    let reused = paths
        .ensure(
            "module-a",
            &mut || {
                starts += 1;
                Ok(())
            },
            Duration::from_secs(5),
        )
        .unwrap();
    assert_eq!(reused, started);
    assert_eq!(starts, 1);
    assert!(!paths.lock().exists());
}

#[test]
fn a_gateway_that_fails_to_start_reports_why() {
    let (_directory, paths) = paths();
    let error = paths
        .ensure(
            "module-a",
            &mut || write_private(&paths.failure(), b"gg crashed\n"),
            Duration::from_secs(5),
        )
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "the Graftcode gateway could not start: gg crashed"
    );
}

#[test]
fn a_gateway_from_another_build_is_asked_to_stop_first() {
    let (_directory, paths) = paths();
    let old = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let old_port = old.local_addr().unwrap().port();
    drop(old);
    paths.write_state(&state(old_port, "module-old")).unwrap();
    let port = status_server();
    let ready = paths
        .ensure(
            "module-new",
            &mut || paths.write_state(&state(port, "module-new")),
            Duration::from_secs(5),
        )
        .unwrap();
    assert_eq!(ready.module, "module-new");
    paths.clear_state(7);
    assert_eq!(paths.read_state(), None);
}

#[test]
fn only_the_owning_daemon_clears_a_stop_request() {
    let (_directory, paths) = paths();
    paths.write_state(&state(1, "module")).unwrap();
    fs::write(paths.stop(), "stop\n").unwrap();
    paths.clear_state(8);
    assert!(paths.stop_requested());
    assert!(paths.read_state().is_some());
    paths.clear_state(7);
    assert!(!paths.stop_requested());
    assert_eq!(paths.read_state(), None);
}

#[test]
fn ports_in_use_are_skipped() {
    let taken = TcpListener::bind(("0.0.0.0", 0)).unwrap();
    let port = taken.local_addr().unwrap().port();
    let found = free_port(port).unwrap();
    assert_ne!(found, port);
    assert_eq!((found - port) % 2, 0);
    assert_eq!(new_token().len(), 48);
    assert_ne!(new_token(), new_token());
}
