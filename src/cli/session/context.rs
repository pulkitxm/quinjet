#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " Files a repository commits to say how it wants to be worked on. These"]
#[doc = " are the one trusted section of a bundle, so the list is fixed here"]
#[doc = " rather than taken from anything a pull request can influence."]
const INSTRUCTION_FILES: [&str; 5] = [
    "AGENTS.md",
    "CLAUDE.md",
    ".github/copilot-instructions.md",
    "CONTRIBUTING.md",
    ".cursorrules",
];
#[doc = " One instruction file past this is a manual, not an instruction, and"]
#[doc = " would crowd out the patch it is meant to explain."]
const MAX_INSTRUCTION_BYTES: u64 = 64 * 1024;

impl Session {
    #[doc = " Assemble a bundle for one purpose. Everything it draws on is fetched"]
    #[doc = " here and reduced by a pure function, so two runs against the same"]
    #[doc = " head produce the same bundle."]
    pub(crate) fn pull_request_context(
        &self,
        pull_request: &PullRequest,
        request: &ContextRequest,
        index: &PullRequestDiffIndex,
        patch: &str,
        merge_base_oid: &str,
    ) -> PullRequestContext {
        let mut warnings = Vec::new();
        let review = Self::optional(
            &mut warnings,
            "review threads",
            self.repository.pull_request_review(pull_request),
        );
        let gate = Self::optional(
            &mut warnings,
            "the merge gate",
            self.repository.pull_request_gate(pull_request, false),
        );
        let annotations = match request.purpose {
            ContextPurpose::Review => None,
            _ => Self::optional(
                &mut warnings,
                "check annotations",
                self.repository
                    .pull_request_annotations(pull_request, false),
            ),
        };
        let dependencies = Self::optional(
            &mut warnings,
            "dependency changes",
            self.repository.pull_request_dependencies(pull_request),
        );
        let commits = Self::optional(
            &mut warnings,
            "the commit list",
            self.repository.pull_request_commits(pull_request),
        );
        let instructions = self.instruction_files();
        build_context(&ContextInputs {
            pull_request,
            purpose: request.purpose,
            budget: request.budget,
            merge_base_oid,
            index,
            patch,
            review: review.as_ref(),
            gate: gate.as_ref(),
            annotations: annotations.as_ref(),
            dependencies: dependencies.as_ref(),
            commits: commits.as_ref(),
            instructions: &instructions,
            generated_at: crate::date_time::now_timestamp(),
            warnings: Vec::new(),
        })
        .with_warnings(warnings)
    }

    #[doc = " A read whose absence is a warning rather than a failure: a bundle"]
    #[doc = " missing one section is worth more than no bundle, as long as it says"]
    #[doc = " what it could not see."]
    fn optional<T>(warnings: &mut Vec<String>, what: &str, result: Result<T>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                warnings.push(format!("{what} were not readable: {error:#}"));
                None
            }
        }
    }

    fn instruction_files(&self) -> Vec<(PathBuf, String)> {
        INSTRUCTION_FILES
            .iter()
            .filter_map(|name| {
                let path = self.repository.root().join(name);
                let length = std::fs::metadata(&path).ok()?.len();
                if length > MAX_INSTRUCTION_BYTES {
                    return None;
                }
                let contents = std::fs::read_to_string(&path).ok()?;
                Some((PathBuf::from(name), contents))
            })
            .collect()
    }
}
