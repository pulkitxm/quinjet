use std::path::{Component, Path};

use super::PROGRAM;

pub(super) fn manages(executable: &Path) -> bool {
    let resolved = executable.canonicalize();
    cellar_installation(resolved.as_deref().unwrap_or(executable))
}

pub(super) fn manages_running_executable() -> bool {
    std::env::current_exe() // nosemgrep: rust.lang.security.current-exe.current-exe
        .is_ok_and(|executable| manages(&executable))
}

fn cellar_installation(path: &Path) -> bool {
    let mut names = path.components().filter_map(|component| match component {
        Component::Normal(name) => name.to_str(),
        _ => None,
    });
    while let Some(name) = names.next() {
        if name == "Cellar" && names.next() == Some(PROGRAM) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_silicon_and_linuxbrew_cellars_are_managed() {
        assert!(cellar_installation(Path::new(
            "/opt/homebrew/Cellar/quinjet/0.0.6/bin/quinjet"
        )));
        assert!(cellar_installation(Path::new(
            "/usr/local/Cellar/quinjet/0.0.6/bin/quinjet"
        )));
        assert!(cellar_installation(Path::new(
            "/home/linuxbrew/.linuxbrew/Cellar/quinjet/0.0.6/bin/quinjet"
        )));
    }

    #[test]
    fn other_installations_are_left_alone() {
        assert!(!cellar_installation(Path::new("/usr/local/bin/quinjet")));
        assert!(!cellar_installation(Path::new(
            "/home/pat/.cargo/bin/quinjet"
        )));
        assert!(!cellar_installation(Path::new(
            "/opt/homebrew/Cellar/git/2.51.0/bin/git"
        )));
        assert!(!cellar_installation(Path::new(
            "/home/pat/Cellar/downloads/quinjet"
        )));
    }
}
