use std::path::Path;
use std::process::Command;

use super::PROGRAM;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Manager {
    pub name: &'static str,
    pub upgrade: &'static str,
}

const APT: Manager = Manager {
    name: "apt",
    upgrade: "sudo apt update && sudo apt install --only-upgrade quinjet",
};
const HOMEBREW: Manager = Manager {
    name: "Homebrew",
    upgrade: "brew upgrade quinjet",
};
const WINGET: Manager = Manager {
    name: "Winget",
    upgrade: "winget upgrade Pulkitxm.Quinjet",
};

pub(super) fn manager(executable: &Path) -> Option<Manager> {
    let resolved = executable.canonicalize();
    let path = resolved.as_deref().unwrap_or(executable);
    path_manager(path).or_else(|| apt_manages(path).then_some(APT))
}

pub(super) fn owns_integrations_for_running_executable() -> bool {
    std::env::current_exe() // nosemgrep: rust.lang.security.current-exe.current-exe
        .is_ok_and(|executable| manager(&executable).is_some_and(|manager| manager != WINGET))
}

fn path_manager(path: &Path) -> Option<Manager> {
    let path_text = path.to_string_lossy();
    let names = path_text
        .split(['/', '\\'])
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    if names
        .windows(2)
        .any(|parts| parts.first() == Some(&"Cellar") && parts.get(1) == Some(&PROGRAM))
    {
        return Some(HOMEBREW);
    }
    if names.windows(4).any(|parts| {
        parts
            .first()
            .is_some_and(|name| name.eq_ignore_ascii_case("Microsoft"))
            && parts
                .get(1)
                .is_some_and(|name| name.eq_ignore_ascii_case("WinGet"))
            && parts
                .get(2)
                .is_some_and(|name| name.eq_ignore_ascii_case("Packages"))
            && parts
                .get(3)
                .is_some_and(|name| name.starts_with("Pulkitxm.Quinjet_"))
    }) {
        return Some(WINGET);
    }
    None
}

fn apt_manages(path: &Path) -> bool {
    if path != Path::new("/usr/bin/quinjet") {
        return false;
    }
    Command::new("dpkg-query")
        .args(["--search", "/usr/bin/quinjet"])
        .output()
        .is_ok_and(|output| {
            output.status.success()
                && apt_query_matches(String::from_utf8_lossy(&output.stdout).as_ref())
        })
}

fn apt_query_matches(output: &str) -> bool {
    output.lines().any(|line| {
        line.split_once(": ").is_some_and(|(package, path)| {
            (package == PROGRAM || package.starts_with("quinjet:")) && path == "/usr/bin/quinjet"
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn homebrew_cellars_are_managed() {
        for path in [
            "/opt/homebrew/Cellar/quinjet/0.0.6/bin/quinjet",
            "/usr/local/Cellar/quinjet/0.0.6/bin/quinjet",
            "/home/linuxbrew/.linuxbrew/Cellar/quinjet/0.0.6/bin/quinjet",
        ] {
            assert_eq!(path_manager(Path::new(path)), Some(HOMEBREW));
        }
    }

    #[test]
    fn winget_package_directory_is_managed() {
        let path = Path::new(
            r"C:\Users\pat\AppData\Local\Microsoft\WinGet\Packages\Pulkitxm.Quinjet_Microsoft.Winget.Source_8wekyb3d8bbwe\quinjet.exe",
        );
        assert_eq!(path_manager(path), Some(WINGET));
    }

    #[test]
    fn apt_query_requires_the_quinjet_package_and_path() {
        assert!(apt_query_matches("quinjet: /usr/bin/quinjet\n"));
        assert!(apt_query_matches("quinjet:amd64: /usr/bin/quinjet\n"));
        assert!(!apt_query_matches("other: /usr/bin/quinjet\n"));
        assert!(!apt_query_matches("quinjet: /usr/local/bin/quinjet\n"));
    }

    #[test]
    fn other_installations_are_left_alone() {
        for path in [
            "/usr/local/bin/quinjet",
            "/home/pat/.cargo/bin/quinjet",
            "/opt/homebrew/Cellar/git/2.51.0/bin/git",
            "/home/pat/Cellar/downloads/quinjet",
            r"C:\Users\pat\.cargo\bin\quinjet.exe",
        ] {
            assert_eq!(path_manager(Path::new(path)), None);
        }
    }

    #[cfg(unix)]
    #[test]
    fn prefix_symlink_resolves_to_the_cellar() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let executable = directory
            .path()
            .join("Cellar")
            .join(PROGRAM)
            .join("1.2.3")
            .join("bin")
            .join(PROGRAM);
        std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
        std::fs::write(&executable, []).unwrap();
        let prefix_bin = directory.path().join("bin");
        std::fs::create_dir_all(&prefix_bin).unwrap();
        let linked = prefix_bin.join(PROGRAM);
        symlink(executable, &linked).unwrap();

        assert_eq!(manager(&linked), Some(HOMEBREW));
    }
}
