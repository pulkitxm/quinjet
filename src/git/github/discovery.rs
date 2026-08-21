use super::*;

impl Repository {
    pub(crate) fn github_repositories(
        &self,
        refresh: bool,
    ) -> Result<(Vec<GitHubRepository>, Vec<String>)> {
        let (remote_urls, mut warnings) = self.remote_urls()?;
        let grouped_remote_urls = group_remote_urls(&remote_urls);
        let mut repositories = BTreeMap::new();

        for (index, (url, remotes)) in grouped_remote_urls.iter().take(MAX_REMOTE_URLS).enumerate()
        {
            match repository_from_remote_url(url)
                .map(|repository| (repository, CacheDisposition::Fresh))
                .map_or_else(|| self.resolve_github_repository(Some(url), refresh), Ok)
            {
                Ok((repository, disposition)) => {
                    if disposition == CacheDisposition::Stale {
                        warnings.push(format!(
                            "Using stale cached GitHub identity for remote{} {}",
                            if remotes.len() == 1 { "" } else { "s" },
                            remotes.join(", ")
                        ));
                    }
                    if remotes.is_empty() {
                        merge_repository(&mut repositories, repository, None);
                    } else {
                        for remote in remotes {
                            merge_repository(&mut repositories, repository.clone(), Some(remote));
                        }
                    }
                }
                Err(error) => warnings.push(format!(
                    "remote{} `{}` {} not available through gh: {error}",
                    if remotes.len() == 1 { "" } else { "s" },
                    remotes.join(", "),
                    if remotes.len() == 1 { "is" } else { "are" }
                )),
            }
            if repositories.len() >= MAX_GITHUB_REPOSITORIES {
                if index + 1 < grouped_remote_urls.len().min(MAX_REMOTE_URLS) {
                    warnings.push(format!(
                        "Only the first {MAX_GITHUB_REPOSITORIES} distinct GitHub repositories were loaded"
                    ));
                }
                break;
            }
        }
        if grouped_remote_urls.len() > MAX_REMOTE_URLS {
            warnings.push(format!(
                "Only the first {MAX_REMOTE_URLS} distinct fetch/push remote URLs were inspected"
            ));
        }

        if repositories.is_empty() {
            match self.resolve_github_repository(None, refresh) {
                Ok((repository, disposition)) => {
                    if disposition == CacheDisposition::Stale {
                        warnings.push("Using a stale cached inferred GitHub repository".to_owned());
                    }
                    merge_repository(&mut repositories, repository, None);
                }
                Err(error) => {
                    let remote_hint = if remote_urls.is_empty() {
                        "No Git remotes are configured".to_owned()
                    } else {
                        "No configured fetch or push remote resolves to GitHub".to_owned()
                    };
                    bail!("{remote_hint}; GitHub CLI could not infer a repository: {error}");
                }
            }
        }

        let mut repositories: Vec<_> = repositories.into_values().collect();
        for repository in &mut repositories {
            repository.remotes.sort();
            repository.remotes.dedup();
        }
        repositories.sort_by_key(|repository| {
            (
                !repository.remotes.iter().any(|remote| remote == "origin"),
                repository.display_name().to_lowercase(),
            )
        });
        Ok((repositories, warnings))
    }

    pub(crate) fn local_github_repository(&self) -> Result<Option<GitHubRepository>> {
        let (remote_urls, _) = self.remote_urls()?;
        let mut repositories = BTreeMap::new();
        for remote_url in remote_urls {
            if let Some(repository) = repository_from_remote_url(&remote_url.url) {
                merge_repository(&mut repositories, repository, Some(&remote_url.remote));
            }
        }
        let mut repositories = repositories.into_values().collect::<Vec<_>>();
        repositories.sort_by_key(|repository| {
            (
                !repository.remotes.iter().any(|remote| remote == "origin"),
                repository.display_name().to_lowercase(),
            )
        });
        Ok(repositories.into_iter().next())
    }

    pub(super) fn remote_urls(&self) -> Result<(Vec<RemoteUrl>, Vec<String>)> {
        let output = self.checked([OsString::from("remote")])?;
        let mut urls = BTreeSet::new();
        let mut warnings = Vec::new();

        let mut remote_count = 0;
        'remotes: for record in output.split(|byte| *byte == b'\n') {
            let record = trim_ascii(record);
            if record.is_empty() {
                continue;
            }
            if remote_count >= MAX_GIT_REMOTES {
                warnings.push(format!(
                    "Only the first {MAX_GIT_REMOTES} Git remotes were inspected"
                ));
                break;
            }
            remote_count += 1;
            let remote = text(record);
            for push in [false, true] {
                let mut args = vec![OsString::from("remote"), OsString::from("get-url")];
                if push {
                    args.push(OsString::from("--push"));
                }
                args.push(OsString::from("--all"));
                args.push(OsString::from(&remote));
                let remote_output = self.run(args)?;
                if !remote_output.status.success() {
                    if !push {
                        warnings.push(format!("Unable to read URL for remote `{remote}`"));
                    }
                    continue;
                }
                for url in remote_output.stdout.split(|byte| *byte == b'\n') {
                    let url = trim_ascii(url);
                    if url.is_empty() {
                        continue;
                    }
                    let entry = (remote.clone(), text(url));
                    if urls.len() >= MAX_REMOTE_URL_ENTRIES && !urls.contains(&entry) {
                        warnings.push(format!(
                            "Only the first {MAX_REMOTE_URL_ENTRIES} configured fetch/push URL entries were inspected"
                        ));
                        break 'remotes;
                    }
                    let _ = urls.insert(entry);
                }
            }
        }

        Ok((
            urls.into_iter()
                .map(|(remote, url)| RemoteUrl { remote, url })
                .collect(),
            warnings,
        ))
    }

    pub(super) fn resolve_github_repository(
        &self,
        url: Option<&str>,
        refresh: bool,
    ) -> Result<(GitHubRepository, CacheDisposition)> {
        let mut args = vec![OsString::from("repo"), OsString::from("view")];
        if let Some(url) = url {
            args.push(OsString::from(url));
        }
        args.extend([
            OsString::from("--json"),
            OsString::from("nameWithOwner,url"),
            OsString::from("--template"),
            OsString::from(REPOSITORY_TSV_TEMPLATE),
        ]);
        let identity = url.map_or_else(
            || {
                format!(
                    "inferred\n{}\n{}",
                    self.root.display(),
                    env::var("GH_REPO").unwrap_or_default()
                )
            },
            remote_url_for_gh,
        );
        let key = format!("repository\n{identity}");
        let response = self.checked_cached_gh(
            &key,
            CacheLife::Ttl(REPOSITORY_CACHE_TTL),
            refresh,
            args,
            "gh repo view failed",
        )?;
        let record = trim_ascii(&response.data);
        let [name_with_owner, host] =
            parse_tsv_record::<2>(record).context("invalid gh repo view output")?;
        if name_with_owner.trim().is_empty() || host.trim().is_empty() {
            bail!("gh repo view returned an incomplete repository identity");
        }
        let fields = [name_with_owner, host];
        Ok((
            GitHubRepository {
                name_with_owner: fields[0].clone(),
                url: fields[1].trim_end_matches('/').to_owned(),
                remotes: Vec::new(),
            },
            response.disposition,
        ))
    }

    pub(super) fn checked_cached_gh<I, S>(
        &self,
        cache_key: &str,
        life: CacheLife,
        refresh: bool,
        args: I,
        error_context: &str,
    ) -> Result<GhResponse>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.checked_cached_gh_bounded(
            cache_key,
            life,
            refresh,
            args,
            error_context,
            MAX_GH_METADATA_BYTES,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the renderer needs the whole row context in one call"
    )]
    /// `limit` bounds both the response Quinjet will read and the entry it will
    /// keep, so a check log can use the cache without letting metadata grow.
    pub(super) fn checked_cached_gh_bounded<I, S>(
        &self,
        cache_key: &str,
        life: CacheLife,
        refresh: bool,
        args: I,
        error_context: &str,
        limit: usize,
    ) -> Result<GhResponse>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let cache = CacheStore::discover();
        let cached = cache
            .as_ref()
            .and_then(|cache| cache.read(cache_key, limit));
        if (!refresh || life == CacheLife::Immutable)
            && let Some(entry) = cached.as_ref()
            && life.accepts(entry.age)
        {
            return Ok(GhResponse {
                data: entry.data.clone(),
                disposition: CacheDisposition::Fresh,
            });
        }

        let output = match self.run_gh_bounded(args, limit) {
            Ok(output) => output,
            Err(error) => {
                if let Some(entry) = cached.as_ref() {
                    return Ok(GhResponse {
                        data: entry.data.clone(),
                        disposition: CacheDisposition::Stale,
                    });
                }
                return Err(error);
            }
        };
        if output.status.success() && !output.stdout_truncated {
            if let Some(cache) = cache.as_ref() {
                drop(cache.write(cache_key, &output.stdout, limit));
            }
            return Ok(GhResponse {
                data: output.stdout,
                disposition: CacheDisposition::Network,
            });
        }
        if let Some(entry) = cached {
            return Ok(GhResponse {
                data: entry.data,
                disposition: CacheDisposition::Stale,
            });
        }
        if output.stdout_truncated {
            bail!("{error_context}: GitHub CLI output exceeded the metadata limit");
        }
        bail!("{}", bounded_command_error(error_context, &output));
    }
}
