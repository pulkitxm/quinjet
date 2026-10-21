use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::theme::Appearance;

fn query(program: &str, args: &[&str]) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_millis(75);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                return child
                    .wait_with_output()
                    .ok()
                    .and_then(|output| String::from_utf8(output.stdout).ok());
            }
            Ok(Some(_)) => return None,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(2)),
            Ok(None) | Err(_) => {
                drop(child.kill());
                drop(child.wait());
                return None;
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn detect() -> Option<Appearance> {
    let output = query("/usr/bin/defaults", &["read", "-g"])?;
    Some(
        if output
            .lines()
            .any(|line| line.trim() == "AppleInterfaceStyle = Dark;")
        {
            Appearance::Dark
        } else {
            Appearance::Light
        },
    )
}

#[cfg(windows)]
pub(crate) fn detect() -> Option<Appearance> {
    let output = query(
        "reg.exe",
        &[
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            "/v",
            "AppsUseLightTheme",
        ],
    )?;
    match output.split_whitespace().last()? {
        "0x0" => Some(Appearance::Dark),
        "0x1" => Some(Appearance::Light),
        _ => None,
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
pub(crate) fn detect() -> Option<Appearance> {
    let output = query(
        "gdbus",
        &[
            "call",
            "--session",
            "--dest",
            "org.freedesktop.portal.Desktop",
            "--object-path",
            "/org/freedesktop/portal/desktop",
            "--method",
            "org.freedesktop.portal.Settings.Read",
            "org.freedesktop.appearance",
            "color-scheme",
        ],
    )
    .or_else(|| {
        query(
            "dbus-send",
            &[
                "--session",
                "--print-reply",
                "--dest=org.freedesktop.portal.Desktop",
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.Settings.Read",
                "string:org.freedesktop.appearance",
                "string:color-scheme",
            ],
        )
    });
    output.as_deref().and_then(portal_appearance).or_else(|| {
        let setting = query(
            "gsettings",
            &["get", "org.gnome.desktop.interface", "color-scheme"],
        )?;
        match setting.trim().trim_matches('\'') {
            "prefer-dark" => Some(Appearance::Dark),
            "prefer-light" => Some(Appearance::Light),
            _ => None,
        }
    })
}

#[cfg(not(any(windows, target_os = "macos")))]
fn portal_appearance(output: &str) -> Option<Appearance> {
    let (_, value) = output.split_once("uint32")?;
    let value = value
        .trim_start()
        .split(|character: char| !character.is_ascii_digit())
        .next()?;
    match value {
        "1" => Some(Appearance::Dark),
        "2" => Some(Appearance::Light),
        _ => None,
    }
}

#[cfg(all(test, not(any(windows, target_os = "macos"))))]
mod tests {
    use super::*;

    #[test]
    fn native_queries_handle_failures_and_terminate_slow_processes() {
        assert_eq!(
            query("/bin/sh", &["-c", "printf light"]),
            Some("light".to_owned())
        );
        assert_eq!(query("/bin/sh", &["-c", "exit 1"]), None);
        assert_eq!(query("/a/missing/theme-utility", &[]), None);
        let started = Instant::now();
        assert_eq!(query("/bin/sh", &["-c", "exec sleep 30"]), None);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn portal_responses_distinguish_dark_light_and_unset() {
        assert_eq!(portal_appearance("(<uint32 1>,)"), Some(Appearance::Dark));
        assert_eq!(
            portal_appearance("variant uint32 2"),
            Some(Appearance::Light)
        );
        assert_eq!(portal_appearance("(<uint32 0>,)"), None);
        assert_eq!(portal_appearance("unavailable"), None);
    }
}
