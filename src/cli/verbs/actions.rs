#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Args)]
#[command(group(clap::ArgGroup::new("rerun-scope").required(true).multiple(false)))]
pub(in crate::cli) struct PrRerunArgs {
    #[command(flatten)]
    pub(in crate::cli) pull_request: PrArgs,
    #[doc = " Rerun only the failed jobs of every failed run"]
    #[arg(long, group = "rerun-scope")]
    pub(in crate::cli) failed: bool,
    #[doc = " Rerun every failed run from the start"]
    #[arg(long, group = "rerun-scope")]
    pub(in crate::cli) all: bool,
    #[doc = " Rerun the one job a named check reported"]
    #[arg(long, value_name = "NAME", group = "rerun-scope", value_hint = ValueHint::Other)]
    pub(in crate::cli) check: Option<String>,
    #[doc = " Confirm; without it the command reports what it would rerun"]
    #[arg(long)]
    pub(in crate::cli) yes: bool,
}

#[derive(Debug, Args)]
pub(in crate::cli) struct PrCancelArgs {
    #[command(flatten)]
    pub(in crate::cli) pull_request: PrArgs,
    #[doc = " Confirm; without it the command reports what it would cancel"]
    #[arg(long)]
    pub(in crate::cli) yes: bool,
}

#[derive(Debug, Subcommand)]
pub(in crate::cli) enum PrArtifactVerb {
    #[doc = " Save one artifact archive next to the working tree"]
    Download(PrArtifactDownloadArgs),
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(in crate::cli) struct PrArtifactsCommand {
    #[command(subcommand)]
    pub(in crate::cli) command: Option<PrArtifactVerb>,
    #[command(flatten)]
    pub(in crate::cli) list: PrOptionalArgs,
}

#[derive(Debug, Args)]
pub(in crate::cli) struct PrArtifactDownloadArgs {
    #[command(flatten)]
    pub(in crate::cli) pull_request: PrArgs,
    #[doc = " Artifact name, or a unique part of one"]
    #[arg(value_name = "NAME", value_hint = ValueHint::Other)]
    pub(in crate::cli) name: String,
    #[doc = " Directory to write the archive into"]
    #[arg(long = "into", value_name = "DIR", default_value = ".", value_hint = ValueHint::DirPath)]
    pub(in crate::cli) directory: PathBuf,
}

#[derive(Debug, Subcommand)]
pub(in crate::cli) enum PrDeploymentVerb {
    #[doc = " Let a waiting environment's runs through"]
    Approve(PrDeploymentReviewArgs),
    #[doc = " Refuse a waiting environment's runs"]
    Reject(PrDeploymentReviewArgs),
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(in crate::cli) struct PrDeploymentsCommand {
    #[command(subcommand)]
    pub(in crate::cli) command: Option<PrDeploymentVerb>,
    #[command(flatten)]
    pub(in crate::cli) list: PrOptionalArgs,
}

#[derive(Debug, Args)]
pub(in crate::cli) struct PrDeploymentReviewArgs {
    #[command(flatten)]
    pub(in crate::cli) pull_request: PrArgs,
    #[doc = " Environment holding the runs"]
    #[arg(value_name = "ENVIRONMENT", value_hint = ValueHint::Other)]
    pub(in crate::cli) environment: String,
    #[doc = " Note to record with the decision"]
    #[arg(long, value_name = "TEXT", default_value = "", value_hint = ValueHint::Other)]
    pub(in crate::cli) comment: String,
    #[doc = " Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(in crate::cli) yes: bool,
}

#[doc = " The pull-request selector for a verb that also carries subcommands with"]
#[doc = " their own number. Clap cannot require the number in both places, so the"]
#[doc = " routing requires it instead."]
#[derive(Debug, Args)]
pub(in crate::cli) struct PrOptionalArgs {
    #[doc = " Pull-request number"]
    #[arg(value_name = "NUMBER", required = false, value_hint = ValueHint::Other)]
    pub(in crate::cli) number: Option<u64>,
    #[doc = " Repository the number belongs to, as owner/name"]
    #[arg(long, value_name = "OWNER/NAME", value_hint = ValueHint::Other)]
    pub(in crate::cli) repo: Option<String>,
    #[doc = " Ask GitHub again instead of answering from the cache"]
    #[arg(long)]
    pub(in crate::cli) refresh: bool,
}

impl PrOptionalArgs {
    pub(in crate::cli) fn pull_request(&self, verb: &str) -> Result<PrArgs> {
        let number = self.number.ok_or_else(|| {
            Failure::new(
                EXIT_USAGE,
                "the following required arguments were not provided:\n  <NUMBER>",
            )
            .hint(format!("run `quinjet {verb} --help` for this verb's usage"))
        })?;
        Ok(PrArgs {
            number,
            repo: self.repo.clone(),
            refresh: self.refresh,
        })
    }
}
