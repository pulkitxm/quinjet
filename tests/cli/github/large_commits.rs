use std::fmt::Write as _;

use super::*;

#[test]
fn github_commit_history_follows_every_page_without_a_commit_limit() -> Result<()> {
    let mut fixture = GitHubFixture::new()?;
    fixture.base_oid = "a".repeat(40);
    fixture.head_oid = format!("{:040x}", 600);
    executable(&fixture.bin.join("gh-default"), GH_SCRIPT)?;
    let mut script = String::from(
        r#"#!/bin/sh
input=$(cat)
case "$input" in
  *'commits(last:100,before:'*)
    printf 'page\n' >> "$FAKE_GH_CAPTURE"
    directory=$(dirname "$0")
    case "$input" in
"#,
    );
    for page in 0..7 {
        let end = 601_usize.saturating_sub(page * 100);
        let start = end.saturating_sub(100);
        let nodes = (start..end)
            .map(|index| {
                serde_json::json!({"commit": {
                    "oid": format!("{index:040x}"),
                    "abbreviatedOid": format!("{index:07x}"),
                    "messageHeadline": format!("commit {index}"),
                    "authoredDate": "2026-08-20T01:00:00Z",
                    "committedDate": "2026-08-20T01:00:00Z",
                    "url": format!("https://github.com/acme/project/commit/{index:040x}"),
                    "author": {"name": "Test Author", "user": null},
                    "committer": null
                }})
            })
            .collect::<Vec<_>>();
        let response = serde_json::json!({"data": {"repository": {"pullRequest": {
            "baseRefOid": fixture.base_oid,
            "headRefOid": fixture.head_oid,
            "commits": {
                "totalCount": 601,
                "nodes": nodes,
                "pageInfo": {"hasPreviousPage": start != 0, "startCursor": format!("page-{}", page + 1)}
            }
        }}}});
        fs::write(
            fixture.bin.join(format!("page-{page}.json")),
            serde_json::to_vec(&response)?,
        )?;
        if page == 0 {
            writeln!(
                script,
                r#"      *'"before":null'*) cat "$directory/page-{page}.json" ;;"#
            )?;
        } else {
            writeln!(
                script,
                r#"      *'"before":"page-{page}"'*) cat "$directory/page-{page}.json" ;;"#
            )?;
        }
    }
    script.push_str(
        r#"      *) exit 91 ;;
    esac
    ;;
  *) printf '%s' "$input" | "$(dirname "$0")/gh-default" "$@" ;;
esac
"#,
    );
    executable(&fixture.bin.join("gh"), &script)?;
    let result = fixture
        .read(&["pr", "commits", "42", "--introduced-only", "--json"])?
        .success()?
        .json()?;
    ensure!(result["totalCommits"] == 601);
    ensure!(
        result["commits"]
            .as_array()
            .is_some_and(|commits| commits.len() == 601)
    );
    ensure!(result["commits"][0]["subject"] == "commit 0");
    ensure!(result["commits"][600]["oid"] == fixture.head_oid);
    ensure!(result["truncated"] == false);
    ensure!(
        fixture
            .gh_calls()?
            .lines()
            .filter(|line| *line == "page")
            .count()
            == 7
    );
    let cached = fixture
        .read(&["pr", "commits", "42", "--introduced-only", "--json"])?
        .success()?
        .json()?;
    ensure!(cached["fromCache"] == true);
    ensure!(cached["commits"] == result["commits"]);
    ensure!(
        fixture
            .gh_calls()?
            .lines()
            .filter(|line| *line == "page")
            .count()
            == 7
    );
    Ok(())
}
