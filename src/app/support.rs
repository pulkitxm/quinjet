use super::*;

pub(super) fn change_list_message(prefix: &str, changes: &[Change]) -> String {
    let mut message = prefix.to_owned();
    let mut listed: Vec<String> = Vec::new();
    for change in changes {
        let label = change.display_path();
        if !listed.contains(&label) {
            listed.push(label);
        }
    }
    for label in &listed {
        message.push_str("\n  ");
        message.push_str(label);
    }
    message
}

pub(super) fn pull_request_loading_document(
    pull_request: &PullRequest,
    message: &str,
) -> DiffDocument {
    let mut document = DiffDocument::empty(
        format!(
            "PR #{} — {}  ·  {} → {}",
            pull_request.number,
            pull_request.title,
            pull_request.head_label(),
            pull_request.base_label(),
        ),
        message,
    );
    document.pull_request_details = Some(pull_request_details(pull_request));
    document
}

pub(super) const fn previous_list_index(selected: usize, length: usize) -> usize {
    if selected == 0 {
        length.saturating_sub(1)
    } else {
        selected.saturating_sub(1)
    }
}

pub(super) const fn next_list_index(selected: usize, length: usize) -> usize {
    if selected.saturating_add(1) >= length {
        0
    } else {
        selected.saturating_add(1)
    }
}

pub(super) fn estimated_patch_bytes(counts: Option<DiffLineCounts>) -> usize {
    counts.map_or(PULL_REQUEST_PATCH_FALLBACK_ESTIMATE, |counts| {
        counts
            .additions
            .saturating_add(counts.deletions)
            .saturating_mul(PULL_REQUEST_PATCH_LINE_ESTIMATE)
            .saturating_add(4_096)
    })
}

pub(super) fn diff_document_size(document: &DiffDocument) -> usize {
    let lines = document.lines.iter().fold(0_usize, |total, line| {
        let spans = line.spans.iter().fold(0_usize, |span_total, span| {
            span_total.saturating_add(size_of_val(span) + span.text.capacity())
        });
        total
            .saturating_add(size_of_val(line))
            .saturating_add(spans)
    });
    size_of_val(document)
        .saturating_add(document.title.capacity())
        .saturating_add(lines)
}

pub(super) fn pull_request_details(pull_request: &PullRequest) -> PullRequestDetails {
    PullRequestDetails {
        number: pull_request.number,
        title: pull_request.title.clone(),
        description: pull_request.description.clone(),
        author: pull_request.author.clone(),
        state: pull_request.state.clone(),
        is_draft: pull_request.is_draft,
        updated_at: pull_request.updated_at.clone(),
        url: pull_request.url.clone(),
        base_repository: pull_request.base_repository.display_name(),
        base_ref: pull_request.base_ref.clone(),
        base_remotes: pull_request.base_repository.remotes.clone(),
        head_repository: pull_request.head_repository.clone(),
        head_ref: pull_request.head_ref.clone(),
        head_remotes: pull_request.head_remotes.clone(),
        is_cross_repository: pull_request.is_cross_repository,
        changed_files: pull_request.changed_files,
        additions: pull_request.additions,
        deletions: pull_request.deletions,
        selected_file: None,
        selected_file_additions: 0,
        selected_file_deletions: 0,
    }
}

pub(super) const fn pull_request_file_status_label(status: PullRequestFileStatus) -> &'static str {
    match status {
        PullRequestFileStatus::Added => "added",
        PullRequestFileStatus::Modified => "modified",
        PullRequestFileStatus::Deleted => "deleted",
        PullRequestFileStatus::Renamed => "renamed",
        PullRequestFileStatus::Copied => "copied",
        PullRequestFileStatus::TypeChanged => "type changed",
        PullRequestFileStatus::Unmerged => "unmerged",
        PullRequestFileStatus::Unknown => "changed",
    }
}

pub(super) fn edit_text(input: &mut TextBuffer, key: KeyEvent, multiline: bool) {
    let word_modifier = key
        .modifiers
        .intersects(KeyModifiers::ALT | KeyModifiers::CONTROL);
    let command_modifier = key
        .modifiers
        .intersects(KeyModifiers::SUPER | KeyModifiers::META | KeyModifiers::HYPER);

    match key.code {
        KeyCode::Backspace if command_modifier => input.delete_to_line_start(),
        KeyCode::Delete if command_modifier => input.delete_to_line_end(),
        KeyCode::Backspace if word_modifier => input.delete_word_backward(),
        KeyCode::Delete if word_modifier => input.delete_word_forward(),
        KeyCode::Left if command_modifier => input.home(),
        KeyCode::Right if command_modifier => input.end(),
        KeyCode::Left if word_modifier => input.move_word_left(),
        KeyCode::Right if word_modifier => input.move_word_right(),
        KeyCode::Home if command_modifier => input.document_start(),
        KeyCode::End if command_modifier => input.document_end(),
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            input.document_start();
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => input.end(),
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => input.move_left(),
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => input.move_right(),
        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            input.delete_word_backward();
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            input.delete_to_line_start();
        }
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            input.delete_to_line_end();
        }
        KeyCode::Char(character)
            if !key.modifiers.intersects(
                KeyModifiers::CONTROL
                    | KeyModifiers::ALT
                    | KeyModifiers::SUPER
                    | KeyModifiers::META
                    | KeyModifiers::HYPER,
            ) =>
        {
            input.insert(character);
        }
        KeyCode::Backspace => input.backspace(),
        KeyCode::Delete => input.delete(),
        KeyCode::Left => input.move_left(),
        KeyCode::Right => input.move_right(),
        KeyCode::Home => input.home(),
        KeyCode::End => input.end(),
        KeyCode::Enter if multiline => input.insert('\n'),
        _ => {}
    }
}

pub(super) fn previous_character(value: &str, cursor: usize) -> Option<(usize, char)> {
    value.get(..cursor)?.char_indices().next_back()
}

pub(super) fn next_character(value: &str, cursor: usize) -> Option<(usize, char)> {
    let character = value.get(cursor..)?.chars().next()?;
    Some((cursor + character.len_utf8(), character))
}

pub(super) fn is_word_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

pub(super) fn repository_root_url(repository: &GitHubRepository) -> Option<&str> {
    repository
        .url
        .trim_end_matches('/')
        .strip_suffix(&repository.name_with_owner)
        .map(|root| root.trim_end_matches('/'))
}

pub(super) fn repository_branch_open_target(repository: &str, branch: &str) -> Option<OpenTarget> {
    if repository.is_empty() || branch.is_empty() {
        return None;
    }
    Some(OpenTarget::Browser(format!(
        "{}/tree/{}",
        repository.trim_end_matches('/'),
        encode_url_path(branch)
    )))
}

pub(super) fn encode_url_path(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            encoded.push(char::from(byte));
        } else if write!(encoded, "%{byte:02X}").is_err() {
            return String::new();
        }
    }
    encoded
}
