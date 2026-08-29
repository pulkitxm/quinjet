#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(in crate::cli) fn whole_document(
    session: &mut Session,
    prepare: Command,
    file: impl Fn(u64, PathBuf) -> Command,
) -> Result<DiffDocument> {
    let index = session.execute(prepare)?.local_diff_index()?;
    let mut loaded = HashMap::new();
    for entry in &index.files {
        let (path, document) = session
            .execute(file(0, entry.path.clone()))?
            .local_diff_file()?;
        drop(loaded.insert(path, document));
    }
    Ok(index.document_with_visibility(&loaded, |_| true))
}
