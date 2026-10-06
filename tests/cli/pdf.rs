use lopdf::{Document, Stream, dictionary};

use super::*;

fn write_pdf(scratch: &Scratch, path: &str, text: &str) -> Result<()> {
    let mut document = Document::with_version("1.5");
    let page_tree_id = document.new_object_id();
    let font_id = document.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
    });
    let resources_id = document.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });
    let mut stream = Stream::new(
        dictionary! {},
        format!("BT\n/F1 12 Tf\n72 720 Td\n({text}) Tj\nET\n")
            .repeat(8)
            .into_bytes(),
    );
    stream.compress()?;
    ensure!(
        stream.dict.has(b"Filter"),
        "fixture must have compressed content"
    );
    let content_id = document.add_object(stream);
    let page_id = document.add_object(dictionary! {
        "Type" => "Page", "Parent" => page_tree_id, "Contents" => content_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    drop(
        document.objects.insert(
            page_tree_id,
            dictionary! {
                "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1,
                "Resources" => resources_id,
            }
            .into(),
        ),
    );
    let catalog_id =
        document.add_object(dictionary! { "Type" => "Catalog", "Pages" => page_tree_id });
    document.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes)?;
    fs::write(scratch.path.join(path), bytes)?;
    Ok(())
}

fn source_diff(scratch: &Scratch, args: &[&str]) -> Result<serde_json::Value> {
    let run = scratch.quinjet(args)?.success()?;
    serde_json::from_str(&run.stdout).context("invalid PDF diff JSON")
}

fn contains_line(document: &serde_json::Value, kind: &str, text: &str) -> bool {
    document["lines"].as_array().is_some_and(|lines| {
        lines.iter().any(|line| {
            line["kind"] == kind
                && line["spans"].as_array().is_some_and(|spans| {
                    spans
                        .iter()
                        .filter_map(|span| span["text"].as_str())
                        .collect::<String>()
                        .contains(text)
                })
        })
    })
}

#[test]
fn pdf_diff_reads_head_index_and_worktree_independently() -> Result<()> {
    let scratch = Scratch::repository()?;
    write_pdf(&scratch, "resume.pdf", "Original version")?;
    scratch.git(&["add", "resume.pdf"])?;
    scratch.git(&["commit", "-m", "Add sample PDF"])?;
    write_pdf(&scratch, "resume.pdf", "Staged version")?;
    scratch.git(&["add", "resume.pdf"])?;
    write_pdf(&scratch, "resume.pdf", "Working version")?;
    let staged = source_diff(&scratch, &["diff", "--staged", "--json"])?;
    ensure!(contains_line(&staged, "removed", "Original version"));
    ensure!(contains_line(&staged, "added", "Staged version"));
    ensure!(!contains_line(&staged, "added", "Working version"));
    let unstaged = source_diff(&scratch, &["diff", "--unstaged", "--json"])?;
    ensure!(contains_line(&unstaged, "removed", "Staged version"));
    ensure!(contains_line(&unstaged, "added", "Working version"));
    let text = scratch.quinjet(&["diff", "--unstaged"])?.success()?.stdout;
    ensure!(text.contains("-(Staged version) Tj"));
    ensure!(text.contains("+(Working version) Tj"));
    ensure!(!text.contains("Binary files"));
    Ok(())
}

#[test]
fn pdf_history_handles_root_commits_renames_and_deletions() -> Result<()> {
    let scratch = Scratch::unborn_repository()?;
    write_pdf(&scratch, "before.pdf", "Initial version")?;
    scratch.git(&["add", "before.pdf"])?;
    scratch.git(&["commit", "-m", "Add sample PDF"])?;
    let root = source_diff(&scratch, &["show", "--json"])?;
    ensure!(contains_line(&root["diff"], "added", "Initial version"));
    scratch.git(&["mv", "before.pdf", "after.pdf"])?;
    write_pdf(&scratch, "after.pdf", "Updated version")?;
    scratch.git(&["add", "after.pdf"])?;
    scratch.git(&["commit", "-m", "Rename and update sample PDF"])?;
    let renamed = source_diff(&scratch, &["show", "--json"])?;
    ensure!(contains_line(
        &renamed["diff"],
        "removed",
        "Initial version"
    ));
    ensure!(contains_line(&renamed["diff"], "added", "Updated version"));
    scratch.git(&["rm", "after.pdf"])?;
    scratch.git(&["commit", "-m", "Delete sample PDF"])?;
    let deleted = source_diff(&scratch, &["show", "--json"])?;
    ensure!(contains_line(
        &deleted["diff"],
        "removed",
        "Updated version"
    ));
    ensure!(!contains_line(&deleted["diff"], "added", "Updated version"));
    Ok(())
}

#[test]
fn pdf_stash_includes_untracked_source() -> Result<()> {
    let scratch = Scratch::repository()?;
    write_pdf(&scratch, "untracked.pdf", "Stashed document")?;
    let untracked = source_diff(&scratch, &["diff", "--json"])?;
    ensure!(contains_line(&untracked, "added", "Stashed document"));
    scratch.git(&[
        "stash",
        "push",
        "--include-untracked",
        "-m",
        "Sample document",
    ])?;
    let stashed = source_diff(&scratch, &["stash", "show", "stash@{0}", "--json"])?;
    ensure!(contains_line(&stashed, "added", "Stashed document"));
    ensure!(!contains_line(&stashed, "meta", "unavailable"));
    Ok(())
}
