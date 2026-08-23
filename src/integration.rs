use clap::ValueEnum;

pub(crate) const HOST_OSC_CODE: u16 = 6973;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Client {
    Edith,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HostAction {
    OpenProjectNewTab,
    OpenWorktreeCurrentTab,
}

impl HostAction {
    const fn payload(self) -> &'static str {
        match self {
            Self::OpenProjectNewTab => "quinjet;open-new-tab",
            Self::OpenWorktreeCurrentTab => "quinjet;open-worktree",
        }
    }

    pub(crate) fn sequence(self) -> String {
        format!("\u{1b}]{HOST_OSC_CODE};{}\u{1b}\\", self.payload())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_actions_use_the_quinjet_osc_channel() {
        assert_eq!(
            HostAction::OpenProjectNewTab.sequence(),
            "\u{1b}]6973;quinjet;open-new-tab\u{1b}\\"
        );
        assert_eq!(
            HostAction::OpenWorktreeCurrentTab.sequence(),
            "\u{1b}]6973;quinjet;open-worktree\u{1b}\\"
        );
    }
}
