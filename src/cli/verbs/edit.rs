#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(in crate::cli) enum PrEditFieldArg {
    Title,
    Body,
    Base,
    AddAssignee,
    RemoveAssignee,
    AddLabel,
    RemoveLabel,
    AddProject,
    RemoveProject,
    AddReviewer,
    RemoveReviewer,
    Milestone,
    RemoveMilestone,
}

#[derive(Debug, Args)]
pub(in crate::cli) struct PrEditArgs {
    #[command(flatten)]
    pub(in crate::cli) pull_request: PrArgs,
    #[doc = " Metadata field or relationship to change"]
    #[arg(value_enum, value_name = "FIELD")]
    pub(in crate::cli) field: PrEditFieldArg,
    #[doc = " New value, or a comma-separated list for relationship fields"]
    #[arg(value_name = "VALUE", value_hint = ValueHint::Other)]
    pub(in crate::cli) value: Option<String>,
    #[doc = " Confirm; without it the command reports what it would do"]
    #[arg(long)]
    pub(in crate::cli) yes: bool,
}

impl PrEditArgs {
    pub(in crate::cli) fn edit(&self) -> Result<PullRequestEdit> {
        if matches!(self.field, PrEditFieldArg::RemoveMilestone) {
            if self.value.is_some() {
                return Err(anyhow::anyhow!("remove-milestone does not take a value"));
            }
            return Ok(PullRequestEdit::RemoveMilestone);
        }
        let value = self
            .value
            .clone()
            .ok_or_else(|| anyhow::anyhow!("the selected edit field needs a value"))?;
        Ok(match self.field {
            PrEditFieldArg::Title => PullRequestEdit::Title(value),
            PrEditFieldArg::Body => PullRequestEdit::Body(value),
            PrEditFieldArg::Base => PullRequestEdit::Base(value),
            PrEditFieldArg::AddAssignee => PullRequestEdit::AddAssignee(value),
            PrEditFieldArg::RemoveAssignee => PullRequestEdit::RemoveAssignee(value),
            PrEditFieldArg::AddLabel => PullRequestEdit::AddLabel(value),
            PrEditFieldArg::RemoveLabel => PullRequestEdit::RemoveLabel(value),
            PrEditFieldArg::AddProject => PullRequestEdit::AddProject(value),
            PrEditFieldArg::RemoveProject => PullRequestEdit::RemoveProject(value),
            PrEditFieldArg::AddReviewer => PullRequestEdit::AddReviewer(value),
            PrEditFieldArg::RemoveReviewer => PullRequestEdit::RemoveReviewer(value),
            PrEditFieldArg::Milestone => PullRequestEdit::SetMilestone(value),
            PrEditFieldArg::RemoveMilestone => return Ok(PullRequestEdit::RemoveMilestone),
        })
    }
}
