use super::*;

#[test]
fn search_finds_change_names_and_file_contents() -> Result<()> {
    let scratch = Scratch::repository()?;
    scratch.write("notes.txt", "unique-search-token\n")?;
    let name = scratch.quinjet(&["search", "notes"])?.success()?;
    ensure!(
        name.stdout.contains("notes.txt"),
        "name search missed the path: {}",
        name.stdout
    );
    let contents = scratch
        .quinjet(&["search", "unique-search-token", "--mode", "contents"])?
        .success()?;
    ensure!(
        contents.stdout.contains("notes.txt"),
        "contents search missed the file: {}",
        contents.stdout
    );
    let history = scratch
        .quinjet(&["search", "base", "--scope", "history", "--mode", "both"])?
        .success()?;
    ensure!(
        !history.stdout.contains("No matches"),
        "history search missed the initial commit: {}",
        history.stdout
    );
    Ok(())
}
