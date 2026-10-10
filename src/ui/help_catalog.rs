#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HelpRow {
    Section(&'static str),
    Shortcut {
        keys: &'static str,
        description: &'static str,
    },
    Spacer,
}

pub(crate) const HELP_ROWS: &[HelpRow] = &[
    HelpRow::Section("Navigation"),
    HelpRow::Shortcut {
        keys: "j / k, ↑ / ↓",
        description: "Move selection or scroll preview",
    },
    HelpRow::Shortcut {
        keys: "Drag in preview",
        description: "Select and copy text inside one diff pane",
    },
    HelpRow::Shortcut {
        keys: "Double-click divider",
        description: "Restore that pane's default size",
    },
    HelpRow::Shortcut {
        keys: "PgUp / PgDn",
        description: "Move by a page",
    },
    HelpRow::Shortcut {
        keys: "Ctrl+D / Ctrl+U",
        description: "Scroll the preview by half a page",
    },
    HelpRow::Shortcut {
        keys: "gg / Home",
        description: "Jump to the first item",
    },
    HelpRow::Shortcut {
        keys: "G / End",
        description: "Jump to the last item",
    },
    HelpRow::Shortcut {
        keys: "Tab",
        description: "Switch focus between sidebar and preview",
    },
    HelpRow::Shortcut {
        keys: "Enter",
        description: "Toggle sidebar / preview focus",
    },
    HelpRow::Shortcut {
        keys: "h / l, ← / →, swipe",
        description: "Scroll preview horizontally",
    },
    HelpRow::Shortcut {
        keys: "[ / ]",
        description: "Previous / next diff hunk",
    },
    HelpRow::Shortcut {
        keys: "e / E",
        description: "Collapse / expand multi-file diffs",
    },
    HelpRow::Shortcut {
        keys: "t / T",
        description: "Toggle expanded vs compact diff context",
    },
    HelpRow::Shortcut {
        keys: "Space in preview",
        description: "Toggle a file in a multi-file preview",
    },
    HelpRow::Shortcut {
        keys: "z",
        description: "Hide / show sidebar",
    },
    HelpRow::Shortcut {
        keys: "m",
        description: "Toggle mouse capture",
    },
    HelpRow::Shortcut {
        keys: "1 / 2 / 3",
        description: "Changes / commit history / pull requests",
    },
    HelpRow::Shortcut {
        keys: "Ctrl+Tab / Ctrl+Shift+Tab",
        description: "Next / previous project tab",
    },
    HelpRow::Shortcut {
        keys: "Option/Alt+1..9",
        description: "Activate a project tab by position",
    },
    HelpRow::Shortcut {
        keys: "Ctrl+W",
        description: "Close the active project tab",
    },
    HelpRow::Shortcut {
        keys: "Right-click project tab",
        description: "Open, close, or close other project tabs",
    },
    HelpRow::Shortcut {
        keys: "N",
        description: "Open a project or worktree in a new tab",
    },
    HelpRow::Shortcut {
        keys: "/",
        description: "Search the active list. Tab chooses Name, Contents, or Both (default Name)",
    },
    HelpRow::Shortcut {
        keys: "Shift+O",
        description: "Open the selected branch, commit, pull request, or check",
    },
    HelpRow::Shortcut {
        keys: "Esc",
        description: "Clear filter, close modal, or return focus",
    },
    HelpRow::Spacer,
    HelpRow::Section("Changes"),
    HelpRow::Shortcut {
        keys: "s / Space",
        description: "Stage or unstage the selected file",
    },
    HelpRow::Shortcut {
        keys: "u",
        description: "Unstage the selected file",
    },
    HelpRow::Shortcut {
        keys: "[+] / [−]",
        description: "Click an individual file or group action",
    },
    HelpRow::Shortcut {
        keys: "Space / ← / →",
        description: "Collapse or expand the selected Changes group",
    },
    HelpRow::Shortcut {
        keys: "a / U",
        description: "Stage all / unstage all",
    },
    HelpRow::Shortcut {
        keys: "x",
        description: "Revert the selected change, or every checked file (asks first)",
    },
    HelpRow::Shortcut {
        keys: "X",
        description: "Remove the selected file, or every checked file (asks first)",
    },
    HelpRow::Shortcut {
        keys: "c",
        description: "Commit staged changes, or stash when files are checked",
    },
    HelpRow::Shortcut {
        keys: "*",
        description: "Check / uncheck the selected file; a group header checkbox does its whole group",
    },
    HelpRow::Shortcut {
        keys: "S",
        description: "View and manage stashes",
    },
    HelpRow::Shortcut {
        keys: "d",
        description: "Compare current branch with another branch",
    },
    HelpRow::Shortcut {
        keys: "b / B",
        description: "Branch picker / checkout branch picker",
    },
    HelpRow::Shortcut {
        keys: "w",
        description: "Switch the current tab to another project or worktree",
    },
    HelpRow::Spacer,
    HelpRow::Section("Commits"),
    HelpRow::Shortcut {
        keys: "b",
        description: "View another local or remote branch (no checkout)",
    },
    HelpRow::Shortcut {
        keys: "C / R",
        description: "Cherry-pick / revert selected commit",
    },
    HelpRow::Shortcut {
        keys: "n",
        description: "Create branch at selected commit",
    },
    HelpRow::Spacer,
    HelpRow::Section("Pull Requests"),
    HelpRow::Shortcut {
        keys: "3",
        description: "Open the on-demand PR view (no automatic fetch)",
    },
    HelpRow::Shortcut {
        keys: "/",
        description: "Focus the numeric PR field; Enter opens it. On Files, search those files",
    },
    HelpRow::Shortcut {
        keys: "o",
        description: "Discover or choose the base repository",
    },
    HelpRow::Shortcut {
        keys: "Shift+P/F/S · Stack p/n, 1..5, r/d",
        description: "PR sections; stack navigation, member tabs, review, and range comparison",
    },
    HelpRow::Shortcut {
        keys: "j / k",
        description: "Select the conversation, a check, a file, or a folder",
    },
    HelpRow::Shortcut {
        keys: "j / k in a file",
        description: "Select a reviewable diff line",
    },
    HelpRow::Shortcut {
        keys: "c / C",
        description: "Add a line comment / file comment to a pending review",
    },
    HelpRow::Shortcut {
        keys: "a / x",
        description: "Reply to / resolve the selected review thread",
    },
    HelpRow::Shortcut {
        keys: "Click review thread",
        description: "Reply, copy, open, edit, delete, or change thread state",
    },
    HelpRow::Shortcut {
        keys: "Shift+V",
        description: "Submit a comment, approval, or change request",
    },
    HelpRow::Shortcut {
        keys: "Space",
        description: "Collapse / expand the selected folder, or open a recent PR",
    },
    HelpRow::Shortcut {
        keys: "r",
        description: "Refetch this PR now, bypassing the cache",
    },
    HelpRow::Shortcut {
        keys: "primary CTA / ▶",
        description: "Merge, close, reopen, or open in browser (after confirm)",
    },
    HelpRow::Spacer,
    HelpRow::Section("Check Logs"),
    HelpRow::Shortcut {
        keys: "j / k in the list",
        description: "Select a check to read its run log",
    },
    HelpRow::Shortcut {
        keys: "Tab, then j / k",
        description: "Move through that run's steps",
    },
    HelpRow::Shortcut {
        keys: "[ / ]",
        description: "Previous / next step",
    },
    HelpRow::Shortcut {
        keys: "Space",
        description: "Fold or unfold the selected step",
    },
    HelpRow::Shortcut {
        keys: "e / E",
        description: "Fold or unfold every step",
    },
    HelpRow::Shortcut {
        keys: "PgUp / PgDn, wheel",
        description: "Scroll the output of an unfolded step",
    },
    HelpRow::Shortcut {
        keys: "h / l, ← / →",
        description: "Read a log line past the right edge",
    },
    HelpRow::Spacer,
    HelpRow::Section("Branches"),
    HelpRow::Shortcut {
        keys: "↑ / ↓",
        description: "Move through matching branches",
    },
    HelpRow::Shortcut {
        keys: "Enter",
        description: "Check out the selected branch",
    },
    HelpRow::Shortcut {
        keys: "Ctrl+N",
        description: "Create a new branch",
    },
    HelpRow::Shortcut {
        keys: "F2 / Ctrl+R",
        description: "Rename the selected branch",
    },
    HelpRow::Shortcut {
        keys: "Delete",
        description: "Delete the selected branch (asks first)",
    },
    HelpRow::Spacer,
    HelpRow::Section("Stashes"),
    HelpRow::Shortcut {
        keys: "↑ / ↓",
        description: "Move through matching stashes",
    },
    HelpRow::Shortcut {
        keys: "Enter",
        description: "Preview the selected stash",
    },
    HelpRow::Shortcut {
        keys: "Ctrl+N",
        description: "Stash working tree changes",
    },
    HelpRow::Shortcut {
        keys: "Ctrl+U",
        description: "Stash including untracked files",
    },
    HelpRow::Shortcut {
        keys: "Ctrl+S",
        description: "Stash only staged changes",
    },
    HelpRow::Shortcut {
        keys: "Alt+A",
        description: "Apply the selected stash",
    },
    HelpRow::Shortcut {
        keys: "Alt+P",
        description: "Pop the selected stash",
    },
    HelpRow::Shortcut {
        keys: "Delete",
        description: "Drop the selected stash (asks first)",
    },
    HelpRow::Shortcut {
        keys: "Ctrl+Delete",
        description: "Clear every stash (asks first)",
    },
    HelpRow::Spacer,
    HelpRow::Section("Conflict"),
    HelpRow::Shortcut {
        keys: "o",
        description: "Keep our version",
    },
    HelpRow::Shortcut {
        keys: "t",
        description: "Keep their version",
    },
    HelpRow::Shortcut {
        keys: "s / Enter",
        description: "Stage the resolved file",
    },
    HelpRow::Spacer,
    HelpRow::Section("Repository"),
    HelpRow::Shortcut {
        keys: "r / Ctrl+R",
        description: "Refresh",
    },
    HelpRow::Shortcut {
        keys: "f / l / p / y",
        description: "Fetch / pull / push / sync",
    },
    HelpRow::Shortcut {
        keys: "Alt+z",
        description: "Toggle code wrapping in either diff layout",
    },
    HelpRow::Shortcut {
        keys: "0",
        description: "Reset horizontal scrolling",
    },
    HelpRow::Shortcut {
        keys: "v",
        description: "Toggle unified / side-by-side diff",
    },
    HelpRow::Shortcut {
        keys: ": / Ctrl+P",
        description: "Open command palette",
    },
    HelpRow::Shortcut {
        keys: "?",
        description: "Show this help",
    },
    HelpRow::Shortcut {
        keys: "q",
        description: "Quit",
    },
];
