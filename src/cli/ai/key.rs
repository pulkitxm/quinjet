use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

const KEY_FILE: &str = "anthropic.json";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredKey {
    api_key: String,
}

pub(crate) fn path() -> Option<PathBuf> {
    crate::state::state_root().map(|root| root.join(KEY_FILE))
}

pub(crate) fn stored() -> Option<String> {
    let stored: StoredKey = serde_json::from_slice(&fs::read(path()?).ok()?).ok()?;
    Some(stored.api_key).filter(|key| !key.trim().is_empty())
}

pub(crate) fn save(key: &str) -> Result<PathBuf> {
    let key = key.trim();
    if key.is_empty() || key.chars().any(char::is_whitespace) {
        bail!("that does not look like an Anthropic API key");
    }
    let path = path().context("could not find Quinjet's state directory")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    drop(fs::remove_file(&path));
    let mut options = OpenOptions::new();
    let _ = options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        let _ = options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .with_context(|| format!("failed to write {}", path.display()))?;
    file.write_all(&serde_json::to_vec_pretty(&StoredKey {
        api_key: key.to_owned(),
    })?)
    .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

pub(crate) fn clear() -> Result<bool> {
    let Some(path) = path() else {
        return Ok(false);
    };
    match fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("failed to remove {}", path.display())),
    }
}

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_stored_privately_and_can_be_cleared() {
        let directory = tempfile::tempdir().unwrap();
        crate::state::with_state_root(directory.path(), || {
            assert_eq!(stored(), None);
            save("  sk-test-123\n").unwrap();
            assert_eq!(stored().as_deref(), Some("sk-test-123"));
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                let mode = fs::metadata(path().unwrap()).unwrap().permissions().mode();
                assert_eq!(mode & 0o777, 0o600);
            }
            save("sk-test-456").unwrap();
            assert_eq!(stored().as_deref(), Some("sk-test-456"));
            save("not a key").unwrap_err();
            assert!(clear().unwrap());
            assert!(!clear().unwrap());
            assert_eq!(stored(), None);
        });
    }
}
