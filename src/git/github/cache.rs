use super::*;

/// Direct cache access for readers that cannot express themselves as a single
/// `gh` invocation: a response judged by its body rather than its exit status,
/// or bytes produced by Git rather than by GitHub.
pub(crate) fn cache_read(key: &str, life: CacheLife) -> Option<Vec<u8>> {
    cache_read_bounded(key, life, MAX_GH_METADATA_BYTES)
}

pub(crate) fn cache_read_bounded(key: &str, life: CacheLife, limit: usize) -> Option<Vec<u8>> {
    CacheStore::discover()?
        .read(key, limit)
        .filter(|entry| life.accepts(entry.age))
        .map(|entry| entry.data)
}

pub(crate) fn cache_write(key: &str, data: &[u8]) {
    cache_write_bounded(key, data, MAX_GH_METADATA_BYTES);
}

pub(crate) fn cache_write_bounded(key: &str, data: &[u8], limit: usize) {
    if let Some(cache) = CacheStore::discover() {
        drop(cache.write(key, data, limit));
    }
}

#[cfg(not(test))]
pub(crate) fn recent_pull_requests() -> Vec<RecentPullRequest> {
    let mut recent = cache_read(RECENT_PULL_REQUESTS_CACHE_KEY, CacheLife::Immutable)
        .and_then(|data| serde_json::from_slice::<Vec<RecentPullRequest>>(&data).ok())
        .unwrap_or_default();
    if let Some(cache) = CacheStore::discover() {
        for pull_request in cache.cached_pull_requests() {
            if recent.iter().any(|existing| {
                existing.number == pull_request.number
                    && existing
                        .repository
                        .url
                        .eq_ignore_ascii_case(&pull_request.repository.url)
            }) {
                continue;
            }
            recent.push(pull_request);
            if recent.len() == MAX_RECENT_PULL_REQUESTS {
                break;
            }
        }
    }
    recent.truncate(MAX_RECENT_PULL_REQUESTS);
    recent
}

#[cfg(not(test))]
pub(crate) fn record_recent_pull_request(pull_request: &PullRequest) -> Vec<RecentPullRequest> {
    let current = RecentPullRequest::from(pull_request);
    let mut recent = recent_pull_requests();
    recent.retain(|existing| {
        existing.number != current.number
            || !existing
                .repository
                .url
                .eq_ignore_ascii_case(&current.repository.url)
    });
    recent.insert(0, current);
    recent.truncate(MAX_RECENT_PULL_REQUESTS);
    if let Ok(data) = serde_json::to_vec(&recent) {
        cache_write(RECENT_PULL_REQUESTS_CACHE_KEY, &data);
    }
    recent
}

pub(super) struct CacheEntry {
    pub(super) data: Vec<u8>,
    pub(super) age: Duration,
}

pub(super) struct CacheStore {
    root: PathBuf,
}

impl CacheStore {
    pub(super) fn discover() -> Option<Self> {
        cache_root().map(|root| Self {
            root: root.join("github"),
        })
    }

    #[cfg(test)]
    pub(super) const fn at(root: PathBuf) -> Self {
        Self { root }
    }

    pub(super) fn read(&self, key: &str, limit: usize) -> Option<CacheEntry> {
        let path = self.path(key);
        let metadata = fs::metadata(&path).ok()?;
        if metadata.len() > limit as u64 + CACHE_MAGIC.len() as u64 {
            drop(fs::remove_file(path));
            return None;
        }
        let mut data = fs::read(path).ok()?;
        if !data.starts_with(CACHE_MAGIC) {
            return None;
        }
        drop(data.drain(..CACHE_MAGIC.len()));
        let age = metadata
            .modified()
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .unwrap_or_default();
        Some(CacheEntry { data, age })
    }

    pub(super) fn write(&self, key: &str, data: &[u8], limit: usize) -> Result<()> {
        if data.len() > limit {
            return Ok(());
        }
        create_private_directory(&self.root)?;
        let destination = self.path(key);
        let id = CACHE_WRITE_ID.fetch_add(1, Ordering::Relaxed);
        let temporary = self
            .root
            .join(format!(".write-{}-{id}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        let _ = options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let _ = options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(CACHE_MAGIC)?;
        file.write_all(data)?;
        file.flush()?;
        drop(file);
        if destination.exists() {
            drop(fs::remove_file(&destination));
        }
        if let Err(error) = fs::rename(&temporary, &destination) {
            drop(fs::remove_file(&temporary));
            return Err(error.into());
        }
        self.prune();
        Ok(())
    }

    pub(super) fn path(&self, key: &str) -> PathBuf {
        let (left, right) = stable_cache_hash(key.as_bytes());
        self.root.join(format!("{left:016x}{right:016x}.cache"))
    }

    pub(super) fn cached_pull_requests(&self) -> Vec<RecentPullRequest> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut files = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().and_then(OsStr::to_str) != Some("cache") {
                    return None;
                }
                let metadata = entry.metadata().ok()?;
                if metadata.len() > MAX_RECENT_CACHE_ENTRY_BYTES {
                    return None;
                }
                Some((metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH), path))
            })
            .collect::<Vec<_>>();
        files.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
        files
            .into_iter()
            .take(MAX_RECENT_CACHE_SCAN)
            .filter_map(|(_, path)| cached_pull_request_at(&path))
            .take(MAX_RECENT_PULL_REQUESTS)
            .collect()
    }

    fn prune(&self) {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return;
        };
        let mut files = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().and_then(OsStr::to_str) != Some("cache") {
                    return None;
                }
                let metadata = entry.metadata().ok()?;
                Some((
                    metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    metadata.len(),
                    path,
                ))
            })
            .collect::<Vec<_>>();
        files.sort_by_key(|(modified, ..)| *modified);
        let mut total = files.iter().map(|(_, bytes, _)| *bytes).sum::<u64>();
        let mut count = files.len();
        for (_, bytes, path) in files {
            if count <= MAX_CACHE_ENTRIES && total <= MAX_CACHE_BYTES {
                break;
            }
            if fs::remove_file(path).is_ok() {
                count = count.saturating_sub(1);
                total = total.saturating_sub(bytes);
            }
        }
    }
}

pub(super) fn cached_pull_request_at(path: &Path) -> Option<RecentPullRequest> {
    let data = fs::read(path).ok()?;
    let body = data.strip_prefix(CACHE_MAGIC)?;
    let record = body
        .split(|byte| *byte == b'\n')
        .find(|record| !record.is_empty())?;
    let fields = parse_tsv_record::<PULL_REQUEST_TSV_FIELDS>(record).ok()?;
    let number = fields.first()?.parse::<u64>().ok()?;
    let url = fields.get(7)?;
    let repository = repository_from_pull_request_url(url, number)?;
    Some(RecentPullRequest {
        number,
        title: bounded_text(fields.get(1)?, MAX_PULL_REQUEST_TITLE_BYTES),
        repository,
    })
}

pub(super) fn repository_from_pull_request_url(url: &str, number: u64) -> Option<GitHubRepository> {
    let suffix = format!("/pull/{number}");
    let repository_url = url.strip_suffix(&suffix)?.trim_end_matches('/');
    let (_, rest) = repository_url.split_once("://")?;
    let (_, path) = rest.split_once('/')?;
    let mut components = path.trim_matches('/').split('/');
    let owner = components.next()?;
    let name = components.next()?;
    if owner.is_empty() || name.is_empty() || components.next().is_some() {
        return None;
    }
    Some(GitHubRepository {
        name_with_owner: format!("{owner}/{name}"),
        url: repository_url.to_owned(),
        remotes: Vec::new(),
    })
}

pub(super) fn cache_root() -> Option<PathBuf> {
    if let Some(path) = env::var_os("QUINJET_CACHE_DIR").filter(|path| !path.is_empty()) {
        return Some(PathBuf::from(path));
    }
    #[cfg(windows)]
    {
        if let Some(path) = env::var_os("LOCALAPPDATA").filter(|path| !path.is_empty()) {
            return Some(PathBuf::from(path).join("quinjet").join("cache"));
        }
    }
    if let Some(path) = env::var_os("XDG_CACHE_HOME").filter(|path| !path.is_empty()) {
        return Some(PathBuf::from(path).join("quinjet"));
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(path) = env::var_os("HOME").filter(|path| !path.is_empty()) {
            return Some(
                PathBuf::from(path)
                    .join("Library")
                    .join("Caches")
                    .join("quinjet"),
            );
        }
    }
    env::var_os("HOME")
        .filter(|path| !path.is_empty())
        .map(|path| PathBuf::from(path).join(".cache").join("quinjet"))
}

pub(super) fn create_private_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub(super) fn stable_cache_hash(value: &[u8]) -> (u64, u64) {
    let mut left = 0xcbf2_9ce4_8422_2325_u64;
    let mut right = 0x8422_2325_cbf2_9ce4_u64;
    for byte in value {
        left ^= u64::from(*byte);
        left = left.wrapping_mul(0x0100_0000_01b3);
        right ^= u64::from(*byte).rotate_left(1);
        right = right.wrapping_mul(0x0100_0000_01b3).rotate_left(5);
    }
    (left, right)
}
