use clap::ArgAction;
use serde::Serialize;

pub(crate) const CLASS_NAME: &str = "Quinjet";

pub(crate) const READ_PATHS: &[&str] = &[
    "status",
    "account list",
    "diff",
    "log",
    "show",
    "search",
    "branch list",
    "branch compare",
    "stash list",
    "stash show",
    "worktree list",
    "repos",
    "pr view",
    "pr files",
    "pr commits",
    "pr diff",
    "pr conversation",
    "pr checks",
    "pr logs",
    "pr reviews show",
    "stack view",
    "stack files",
    "stack diff",
];

pub(crate) const HOST_ONLY_PATHS: &[&str] = &[
    "tui",
    "account switch",
    "project list",
    "remote list",
    "remote forget",
    "pr open",
    "completions",
    "man",
    "capabilities",
    "update",
    "ai key",
];

pub(crate) const AI_PATHS: &[&str] = &[
    "ai commit-message",
    "ai review",
    "ai explain-check",
    "ai investigate",
];

pub(crate) const HOST_ONLY_GROUPS: &[&str] = &["graft", "mcp"];

pub(crate) const REPO_PARAM: &str = "repoPath";
pub(crate) const TOKEN_PARAM: &str = "accessToken";
pub(crate) const CONFIRM_PARAM: &str = "yes";

const SKIPPED_ARGUMENTS: &[&str] = &[
    "help",
    "version",
    "repository",
    "remote",
    "ssh_control_path",
    "json",
    "client",
    "pull_request",
    "watch",
    "interval",
    "exit_code",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Access {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Kind {
    Positional,
    Option,
    Switch,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Param {
    pub name: String,
    #[serde(skip)]
    pub help: String,
    pub kind: Kind,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub flag: String,
    pub list: bool,
    #[serde(skip)]
    pub required: bool,
    #[serde(skip)]
    pub choices: Vec<String>,
    #[serde(skip)]
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Method {
    #[serde(skip)]
    pub name: String,
    pub path: Vec<String>,
    #[serde(skip)]
    pub about: String,
    #[serde(skip)]
    pub access: Access,
    pub params: Vec<Param>,
}

#[derive(Debug, Clone)]
pub(crate) struct Receiver {
    pub methods: Vec<Method>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolSummary {
    pub name: String,
    pub command: String,
    pub access: Access,
    pub parameters: Vec<String>,
}

pub(crate) fn access(path: &str) -> Option<Access> {
    let group = path.split(' ').next().unwrap_or_default();
    if HOST_ONLY_PATHS.contains(&path) || HOST_ONLY_GROUPS.contains(&group) {
        None
    } else if READ_PATHS.contains(&path) || AI_PATHS.contains(&path) {
        Some(Access::Read)
    } else {
        Some(Access::Write)
    }
}

pub(crate) fn leaf_paths(command: &clap::Command) -> Vec<(Vec<String>, clap::Command)> {
    let mut leaves = Vec::new();
    collect_leaves(command, &[], &mut leaves);
    leaves
}

fn collect_leaves(
    command: &clap::Command,
    parent: &[String],
    leaves: &mut Vec<(Vec<String>, clap::Command)>,
) {
    for child in command
        .get_subcommands()
        .filter(|child| child.get_name() != "help")
    {
        let mut path = parent.to_vec();
        path.push(child.get_name().to_owned());
        if child
            .get_subcommands()
            .any(|nested| nested.get_name() != "help")
        {
            collect_leaves(child, &path, leaves);
        } else {
            leaves.push((path, child.clone()));
        }
    }
}

impl Receiver {
    pub(crate) fn from_command(root: &clap::Command, writes: bool) -> Self {
        let mut root = root.clone();
        root.build();
        let methods = leaf_paths(&root)
            .into_iter()
            .filter_map(|(path, command)| {
                let access = access(&path.join(" "))?;
                (writes || access == Access::Read).then(|| method(path, &command, access))
            })
            .collect();
        Self { methods }
    }

    pub(crate) fn tools(&self) -> Vec<ToolSummary> {
        self.methods
            .iter()
            .map(|method| ToolSummary {
                name: method.name.clone(),
                command: method.path.join(" "),
                access: method.access,
                parameters: method
                    .params
                    .iter()
                    .map(|param| param.name.clone())
                    .collect(),
            })
            .collect()
    }

    pub(crate) fn investigation_methods(&self) -> impl Iterator<Item = &Method> {
        self.methods
            .iter()
            .filter(|method| method.access == Access::Read && !method.is_ai())
    }

    pub(crate) fn find(&self, name: &str) -> Option<&Method> {
        self.methods.iter().find(|method| method.name == name)
    }
}

fn method(path: Vec<String>, command: &clap::Command, access: Access) -> Method {
    let mut params: Vec<Param> = command
        .get_arguments()
        .filter(|argument| !argument.is_hide_set())
        .filter(|argument| !SKIPPED_ARGUMENTS.contains(&argument.get_id().as_str()))
        .map(param)
        .collect();
    params.sort_by_key(|param| !param.required);
    Method {
        name: camel_case(&path),
        about: command
            .get_about()
            .map(ToString::to_string)
            .unwrap_or_default(),
        path,
        access,
        params,
    }
}

fn param(argument: &clap::Arg) -> Param {
    let list = argument
        .get_num_args()
        .is_some_and(|range| range.max_values() > 1)
        || matches!(argument.get_action(), ArgAction::Append);
    let switch = matches!(argument.get_action(), ArgAction::SetTrue);
    let (kind, flag) = if argument.is_positional() {
        (Kind::Positional, String::new())
    } else {
        let long = argument
            .get_long()
            .unwrap_or_else(|| argument.get_id().as_str());
        (
            if switch { Kind::Switch } else { Kind::Option },
            format!("--{long}"),
        )
    };
    let id_parts: Vec<String> = argument
        .get_id()
        .as_str()
        .split('_')
        .map(str::to_owned)
        .collect();
    Param {
        name: camel_case(&id_parts),
        help: argument
            .get_help()
            .map(ToString::to_string)
            .unwrap_or_default(),
        kind,
        flag,
        list,
        required: argument.is_required_set(),
        choices: argument
            .get_possible_values()
            .iter()
            .filter(|_| !switch)
            .map(|value| value.get_name().to_owned())
            .collect(),
        default: argument
            .get_default_values()
            .first()
            .map(|value| value.to_string_lossy().into_owned()),
    }
}

impl Param {
    pub(crate) fn description(&self) -> String {
        let param = self;
        let mut text = if param.help.is_empty() {
            param.name.clone()
        } else {
            param.help.clone()
        };
        if param.kind == Kind::Switch {
            text.push_str(". true or false");
        }
        if !param.choices.is_empty() {
            text.push_str(". One of: ");
            text.push_str(&param.choices.join(", "));
        }
        if param.list {
            text.push_str(". Several values are separated by spaces");
        }
        if param.required {
            text.push_str(". Required");
        } else {
            match &param.default {
                Some(default) if param.kind != Kind::Switch => {
                    text.push_str(". Optional, pass \"\" for the default ");
                    text.push_str(default);
                }
                _ if param.kind == Kind::Switch => text.push_str(". Optional, false by default"),
                _ => text.push_str(". Optional, pass \"\" to leave it out"),
            }
        }
        text
    }
}

impl Method {
    pub(crate) fn is_ai(&self) -> bool {
        self.path.first().is_some_and(|group| group == "ai")
    }

    pub(crate) fn confirms(&self) -> bool {
        self.params.iter().any(|param| param.name == CONFIRM_PARAM)
    }
}

fn camel_case(parts: &[String]) -> String {
    let mut name = String::new();
    for (index, word) in parts
        .iter()
        .flat_map(|part| part.split(['-', '_']))
        .filter(|word| !word.is_empty())
        .enumerate()
    {
        let mut characters = word.chars();
        if let Some(first) = characters.next() {
            if index == 0 {
                name.extend(first.to_lowercase());
            } else {
                name.extend(first.to_uppercase());
            }
            name.push_str(characters.as_str());
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    #[test]
    fn method_names_are_camel_case_command_paths() {
        assert_eq!(camel_case(&words(&["pr", "checks"])), "prChecks");
        assert_eq!(camel_case(&words(&["cherry-pick"])), "cherryPick");
        assert_eq!(
            camel_case(&words(&["pr", "allow-maintainer-edits"])),
            "prAllowMaintainerEdits"
        );
        assert_eq!(
            camel_case(&words(&["include_untracked"])),
            "includeUntracked"
        );
    }

    #[test]
    fn parameter_names_are_valid_javascript_identifiers() {
        const RESERVED: &[&str] = &[
            "await",
            "break",
            "case",
            "catch",
            "class",
            "const",
            "continue",
            "debugger",
            "default",
            "delete",
            "do",
            "else",
            "enum",
            "export",
            "extends",
            "false",
            "finally",
            "for",
            "function",
            "if",
            "implements",
            "import",
            "in",
            "instanceof",
            "interface",
            "let",
            "new",
            "null",
            "package",
            "private",
            "protected",
            "public",
            "return",
            "static",
            "super",
            "switch",
            "this",
            "throw",
            "true",
            "try",
            "typeof",
            "var",
            "void",
            "while",
            "with",
            "yield",
            "arguments",
            "eval",
        ];
        for method in crate::cli::graft_receiver(true).methods {
            for param in &method.params {
                assert!(
                    !RESERVED.contains(&param.name.as_str()),
                    "{}({}) is a JavaScript reserved word",
                    method.name,
                    param.name
                );
            }
        }
    }

    #[test]
    fn host_only_and_graft_paths_are_never_exposed() {
        assert_eq!(access("tui"), None);
        assert_eq!(access("graft serve"), None);
        assert_eq!(access("pr open"), None);
        assert_eq!(access("pr checks"), Some(Access::Read));
        assert_eq!(access("commit"), Some(Access::Write));
        assert_eq!(access("ai review"), Some(Access::Read));
        assert_eq!(access("ai key"), None);
        assert_eq!(access("mcp install"), None);
    }
}
