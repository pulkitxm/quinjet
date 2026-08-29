#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(in crate::cli) fn prepared_pull_request_diff(
    session: &mut Session,
    out: &Emitter,
    index: &PullRequestDiffIndex,
    title: String,
    path: Option<&Path>,
) -> Result<DiffDocument> {
    let paths: Vec<PathBuf> = match path {
        Some(wanted) => {
            if !index.files.iter().any(|file| file.path == wanted) {
                return Err(Failure::new(
                    EXIT_NOT_FOUND,
                    format!("`{}` is not part of this pull request", wanted.display()),
                )
                .hint("list the files in this pull-request comparison first")
                .into());
            }
            vec![wanted.to_path_buf()]
        }
        None => index.files.iter().map(|file| file.path.clone()).collect(),
    };
    let mut loaded = HashMap::new();
    for chunk in paths.chunks(16) {
        for (path, document) in out
            .execute(
                session,
                Command::PullRequestFileBatch {
                    workspace: 0,
                    paths: chunk.to_vec(),
                },
            )?
            .pull_request_diff_batch()?
        {
            drop(loaded.insert(path, document));
        }
    }
    let index = DiffIndex {
        title,
        files: index
            .files
            .iter()
            .filter(|file| loaded.contains_key(&file.path))
            .map(|file| crate::git::diff::DiffFileIndexEntry {
                path: file.path.clone(),
                old_path: file.old_path.clone(),
                status: render::pull_request_file_label(file.status).to_owned(),
                counts: file.counts,
            })
            .collect(),
        truncated: index.truncated,
        commit_details: None,
    };
    Ok(index.document_with_visibility(&loaded, |_| true))
}
