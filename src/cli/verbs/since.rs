#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Args, Default)]
#[group(multiple = false)]
pub(in crate::cli) struct PrSinceArgs {
    #[doc = " Print only what changed after this commit of the pull request"]
    #[arg(long, value_name = "OID", value_hint = ValueHint::Other)]
    pub(in crate::cli) since: Option<String>,
    #[doc = " Print only what changed since your last visit or review"]
    #[arg(long)]
    pub(in crate::cli) since_review: bool,
}

impl PrSinceArgs {
    pub(in crate::cli) fn request(&self) -> Option<ReviewSinceRequest> {
        match (&self.since, self.since_review) {
            (Some(oid), _) => Some(ReviewSinceRequest::Commit(oid.clone())),
            (None, true) => Some(ReviewSinceRequest::LastReview),
            (None, false) => None,
        }
    }
}
