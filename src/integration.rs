use clap::ValueEnum;

pub(crate) const HOST_OSC_CODE: u16 = 6973;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Client {
    Edith,
}

pub(crate) fn requests_edith_client(args: impl IntoIterator<Item = String>) -> bool {
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        if argument == "--client=edith" {
            return true;
        }
        if argument == "--client" && args.next().as_deref() == Some("edith") {
            return true;
        }
    }
    false
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

    #[test]
    fn edith_client_detection_accepts_both_clap_spellings() {
        for arguments in [
            vec!["quinjet".to_owned(), "--client=edith".to_owned()],
            vec![
                "quinjet".to_owned(),
                "--client".to_owned(),
                "edith".to_owned(),
            ],
        ] {
            assert!(requests_edith_client(arguments));
        }
        assert!(!requests_edith_client([
            "quinjet".to_owned(),
            "--version".to_owned(),
        ]));
    }
}
