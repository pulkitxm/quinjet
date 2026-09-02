#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " What the pull request does to the dependency graph, as GitHub's own"]
#[doc = " comparison of the base and the head sees it."]
pub(super) fn dependencies(session: &mut Session, out: &Emitter, args: &PrArgs) -> Result<u8> {
    let request = lookup(session, out, args)?;
    let listing = out
        .execute(
            session,
            Command::PullRequestDependencies {
                pull_request: Box::new(request),
            },
        )?
        .dependencies()?;
    out.emit(&listing, || render::dependencies(&listing))?;
    Ok(0)
}

#[doc = " The findings a pull request raises, which is the vulnerable"]
#[doc = " dependencies it introduces and the code scanning alerts on its head."]
pub(super) fn security(session: &mut Session, out: &Emitter, args: &PrArgs) -> Result<u8> {
    let request = lookup(session, out, args)?;
    let findings = out
        .execute(
            session,
            Command::PullRequestSecurity {
                pull_request: Box::new(request),
            },
        )?
        .security()?;
    out.emit(&findings, || render::security(&findings))?;
    Ok(if findings.is_serious() {
        EXIT_FAILURE
    } else {
        0
    })
}

#[doc = " Everything a coding or review tool needs for one purpose, in one"]
#[doc = " document, with the repository's own instructions kept apart from text"]
#[doc = " anybody who can comment on the pull request could have written."]
pub(super) fn context(session: &mut Session, out: &Emitter, args: &PrContextArgs) -> Result<u8> {
    let request = lookup(session, out, &args.pull_request)?;
    let bundle = out
        .execute(
            session,
            Command::PullRequestContext {
                pull_request: Box::new(request),
                request: Box::new(ContextRequest {
                    purpose: args.purpose.purpose(),
                    budget: args.budget,
                    path: args.path.clone(),
                }),
            },
        )?
        .context()?;
    let bundle = note_missing_primary_section(bundle, args.purpose.purpose());
    out.emit(&bundle, || render::context(&bundle))?;
    Ok(0)
}

#[doc = " Say so when the budget could not hold the very section the purpose"]
#[doc = " asked for, rather than handing back a bundle that quietly answers a"]
#[doc = " different question."]
fn note_missing_primary_section(
    bundle: PullRequestContext,
    purpose: ContextPurpose,
) -> PullRequestContext {
    let wanted = purpose.primary_section();
    let missing = match bundle.section(wanted) {
        None => format!("the {} section did not fit the budget", wanted.heading()),
        Some(section) if section.is_truncated() => {
            format!("the {} section was cut to fit the budget", wanted.heading())
        }
        Some(_) => return bundle,
    };
    bundle.with_warnings(vec![missing])
}
