use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Result, anyhow};

use crate::graft::gateway::{self, GatewayProcess, ServePlan};

pub(super) fn wait_until_ready(
    plan: &ServePlan,
    process: &mut GatewayProcess,
    limit: Duration,
) -> Result<()> {
    gateway::wait_until_ready(plan.port, &mut process.0, &plan.log, limit).map_err(|error| {
        let missing = missing_libraries(&plan.module_dir);
        if missing.is_empty() {
            error
        } else {
            anyhow!(describe(&missing, &plan.log))
        }
    })
}

fn describe(missing: &[String], log: &Path) -> String {
    let mut text = String::from("the Graftcode gateway's bundled Node.js runtime needs ");
    text.push_str(&missing.join(", "));
    text.push_str(", which this system does not have");
    if missing.iter().any(|name| name.starts_with("libatomic")) {
        text.push_str(
            "; install it with `sudo apt install libatomic1` on Debian and Ubuntu, \
             `sudo dnf install libatomic` on Fedora, or `sudo zypper install libatomic1` on openSUSE",
        );
    }
    text.push_str(" (log: ");
    text.push_str(&log.display().to_string());
    text.push(')');
    text
}

fn missing_libraries(module_dir: &Path) -> Vec<String> {
    if !cfg!(target_os = "linux") {
        return Vec::new();
    }
    let mut missing: Vec<String> = node_libraries(&module_dir.join(".graftcode"))
        .iter()
        .filter_map(|library| {
            Command::new("ldd")
                .arg(library)
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .ok()
        })
        .flat_map(|output| unresolved(&String::from_utf8_lossy(&output.stdout)))
        .collect();
    missing.sort();
    missing.dedup();
    missing
}

fn node_libraries(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut found = Vec::new();
    while let Some(folder) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for path in entries.filter_map(|entry| entry.ok().map(|entry| entry.path())) {
            if path.is_dir() {
                pending.push(path);
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("libnode.so"))
            {
                found.push(path);
            }
        }
    }
    found
}

fn unresolved(ldd: &str) -> Vec<String> {
    ldd.lines()
        .filter(|line| line.contains("=> not found"))
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unresolved_libraries_are_named_with_an_install_command() {
        let ldd = "\tlinux-vdso.so.1 (0x00007ffd)\n\
                   \tlibatomic.so.1 => not found\n\
                   \tlibstdc++.so.6 => /lib/x86_64-linux-gnu/libstdc++.so.6 (0x0000)\n";
        let missing = unresolved(ldd);
        assert_eq!(missing, vec!["libatomic.so.1".to_owned()]);
        let text = describe(&missing, Path::new("/cache/gateway.log"));
        assert!(text.starts_with(
            "the Graftcode gateway's bundled Node.js runtime needs libatomic.so.1, which"
        ));
        assert!(text.contains("`sudo apt install libatomic1`"));
        assert!(text.ends_with("(log: /cache/gateway.log)"));
    }

    #[test]
    fn bundled_node_libraries_are_found_in_the_extracted_runtime() {
        let directory = tempfile::tempdir().unwrap();
        let runtime = directory.path().join("1.5.0/Binaries/Native/Linux/X64");
        std::fs::create_dir_all(&runtime).unwrap();
        std::fs::write(runtime.join("libnode.so.127"), b"").unwrap();
        std::fs::write(runtime.join("libHypertubeConfigs.so"), b"").unwrap();
        assert_eq!(
            node_libraries(directory.path()),
            vec![runtime.join("libnode.so.127")]
        );
    }
}
