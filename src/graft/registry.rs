use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub(crate) const REGISTRY_FILE: &str = "grafts.json";
const MAX_NAME_LENGTH: usize = 40;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Gateway {
    pub name: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_env: Option<String>,
}

impl Gateway {
    pub(crate) fn token(&self) -> Option<String> {
        self.token_env
            .as_deref()
            .and_then(|name| std::env::var(name).ok())
            .filter(|token| !token.is_empty())
    }
}

pub(crate) fn registry_path() -> Result<PathBuf> {
    crate::state::state_root()
        .map(|root| root.join(REGISTRY_FILE))
        .context("could not find a state directory; set QUINJET_STATE_DIR")
}

pub(crate) fn load(path: &Path) -> Result<Vec<Gateway>> {
    match fs::read(path) {
        Ok(data) => serde_json::from_slice(&data)
            .with_context(|| format!("{} is not a valid gateway list", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

pub(crate) fn save(path: &Path, gateways: &[Gateway]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let staging = path.with_extension("json.tmp");
    fs::write(&staging, serde_json::to_vec_pretty(gateways)?)
        .with_context(|| format!("failed to write {}", staging.display()))?;
    fs::rename(&staging, path).with_context(|| format!("failed to replace {}", path.display()))
}

pub(crate) fn validate_name(name: &str) -> Result<()> {
    let valid = !name.is_empty()
        && name.len() <= MAX_NAME_LENGTH
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if valid {
        Ok(())
    } else {
        bail!("gateway names use letters, digits, `-` and `_`, up to {MAX_NAME_LENGTH} characters")
    }
}

pub(crate) fn endpoint(url: &str) -> Result<String> {
    let trimmed = url.trim().trim_end_matches('/');
    if trimmed.starts_with("https://") {
        bail!(
            "`{url}` uses TLS, which Quinjet does not link; reach a remote gateway through `ssh -L 7390:localhost:7390 <host>` and add http://localhost:7390"
        );
    }
    let rest = trimmed.strip_prefix("http://");
    if rest.is_none_or(|rest| rest.is_empty() || rest.starts_with('/')) {
        bail!("`{url}` is not an http:// gateway address");
    }
    let base = trimmed
        .strip_suffix("/GV")
        .or_else(|| trimmed.strip_suffix("/ws"))
        .unwrap_or(trimmed);
    if base.ends_with("/mcp") {
        Ok(base.to_owned())
    } else {
        Ok(format!("{base}/mcp"))
    }
}

pub(crate) fn upsert(gateways: &mut Vec<Gateway>, gateway: Gateway) -> bool {
    if let Some(existing) = gateways
        .iter_mut()
        .find(|existing| existing.name == gateway.name)
    {
        *existing = gateway;
        false
    } else {
        gateways.push(gateway);
        gateways.sort_by(|left, right| left.name.cmp(&right.name));
        true
    }
}

pub(crate) fn find<'a>(gateways: &'a [Gateway], name: &str) -> Result<&'a Gateway> {
    gateways
        .iter()
        .find(|gateway| gateway.name == name)
        .with_context(|| {
            if gateways.is_empty() {
                format!("no gateway is called `{name}`; none are registered yet")
            } else {
                format!(
                    "no gateway is called `{name}`; registered: {}",
                    gateways
                        .iter()
                        .map(|gateway| gateway.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        })
}

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests {
    use super::*;

    fn gateway(name: &str) -> Gateway {
        Gateway {
            name: name.to_owned(),
            url: format!("http://localhost:7381/{name}/mcp"),
            token_env: None,
        }
    }

    #[test]
    fn endpoints_accept_gateway_vision_and_socket_addresses() {
        for url in [
            "http://localhost:7381",
            "http://localhost:7381/",
            "http://localhost:7381/mcp",
            "http://localhost:7381/GV",
            "http://localhost:7381/ws",
        ] {
            assert_eq!(
                endpoint(url).ok().as_deref(),
                Some("http://localhost:7381/mcp")
            );
        }
        endpoint("ws://localhost:7381").unwrap_err();
        endpoint("https://").unwrap_err();
        let remote = endpoint("https://gateway.example/mcp").unwrap_err().to_string();
        assert!(remote.contains("ssh -L 7390:localhost:7390"), "{remote}");
        endpoint("localhost:7381").unwrap_err();
    }

    #[test]
    fn names_are_short_and_shell_safe() {
        validate_name("agent_1-ci").unwrap();
        validate_name("").unwrap_err();
        validate_name("has space").unwrap_err();
        validate_name(&"x".repeat(41)).unwrap_err();
    }

    #[test]
    fn upsert_replaces_by_name_and_keeps_order() {
        let mut gateways = vec![gateway("quinjet")];
        assert!(upsert(&mut gateways, gateway("agent")));
        assert!(!upsert(&mut gateways, gateway("quinjet")));
        let names: Vec<&str> = gateways.iter().map(|item| item.name.as_str()).collect();
        assert_eq!(names, ["agent", "quinjet"]);
        find(&gateways, "agent").unwrap();
        let missing = find(&gateways, "nope").err().map(|error| error.to_string());
        assert_eq!(
            missing.as_deref(),
            Some("no gateway is called `nope`; registered: agent, quinjet")
        );
    }

    #[test]
    fn the_registry_round_trips_and_starts_empty() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nested").join(REGISTRY_FILE);
        assert_eq!(load(&path).unwrap(), Vec::new());
        let mut gateways = vec![gateway("agent")];
        gateways[0].token_env = Some("AGENT_TOKEN".to_owned());
        save(&path, &gateways).unwrap();
        assert_eq!(load(&path).unwrap(), gateways);
        fs::write(&path, b"not json").unwrap();
        load(&path).unwrap_err();
    }
}
