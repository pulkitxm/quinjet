#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(crate) fn dispatch() -> Result<Launch> {
    completion::auto_install();
    let cli = Cli::parse();
    if let Some(target) = cli.remote.as_deref() {
        let (terminal, implicit_terminal, folder) = match cli.command.as_ref() {
            None => (true, true, cli.repository.as_path()),
            Some(Verb::Tui(args)) => (true, false, terminal_path(&args.path, &cli.repository)),
            Some(_) => (false, false, cli.repository.as_path()),
        };
        return remote::run(
            target,
            terminal,
            implicit_terminal,
            folder,
            cli.ssh_control_path.as_deref(),
        )
        .map(Launch::Finished);
    }
    let mut out = Emitter::new(cli.json);
    let verb = match cli.command {
        None => {
            return Ok(Launch::Terminal(Box::new(TerminalOptions {
                path: cli.repository,
                no_mouse: false,
                webhook_listen: None,
                theme: ThemeName::default().into(),
                appearance: AppearanceChoice::default(),
                pull_request: cli.pull_request,
                client: cli.client,
            })));
        }
        Some(Verb::Tui(args)) => {
            return Ok(Launch::Terminal(Box::new(TerminalOptions {
                path: terminal_path(&args.path, &cli.repository).to_path_buf(),
                no_mouse: args.no_mouse,
                webhook_listen: args.webhook_listen,
                theme: args
                    .theme_palette
                    .map_or_else(|| args.theme.into(), ThemeSelection::Host),
                appearance: args.appearance,
                pull_request: args.pull_request.or(cli.pull_request),
                client: cli.client,
            })));
        }
        Some(Verb::Completions(args)) => {
            return completions(&out, &args).map(Launch::Finished);
        }
        Some(Verb::Man(args)) => return manual(&out, &args).map(Launch::Finished),
        Some(Verb::Capabilities) => return capabilities(&out).map(Launch::Finished),
        Some(Verb::Remote { command }) => {
            return remote::manage(&out, command).map(Launch::Finished);
        }
        Some(Verb::Project { command }) => {
            return projects(&out, command).map(Launch::Finished);
        }
        Some(Verb::Update(args)) => {
            out.start_progress("Checking for updates")?;
            let result = update::run(&out, args.check);
            out.finish_progress();
            return result.map(Launch::Finished);
        }
        Some(other) => other,
    };
    if cli.pull_request.is_some() {
        return Err(Failure::new(
            EXIT_FAILURE,
            "--pr only applies to the terminal interface; use `quinjet pr view <number>` instead",
        )
        .into());
    }
    if let Some(label) = verb.progress_label() {
        out.start_progress(label)?;
    }
    let result = (|| {
        let repository = Repository::discover(&cli.repository)?;
        let mut session = Session::new(repository);
        run(&mut session, &out, verb)
    })();
    out.finish_progress();
    result.map(Launch::Finished)
}

fn terminal_path<'a>(positional: &'a Path, repository: &'a Path) -> &'a Path {
    if positional == Path::new(".") {
        repository
    } else {
        positional
    }
}

fn projects(out: &Emitter, command: ProjectVerb) -> Result<u8> {
    match command {
        ProjectVerb::List => {
            let projects = crate::state::load_stored_projects();
            out.emit(&projects, || render::projects(&projects))?;
            Ok(0)
        }
    }
}

pub(super) fn completions(out: &Emitter, args: &CompletionsArgs) -> Result<u8> {
    let shell = args
        .shell
        .or_else(completion::detected_shell)
        .context("could not detect a supported shell; name one explicitly")?;
    if args.install {
        let paths = if args.automatic {
            completion::maintain(shell)?
        } else {
            completion::install(shell)?
        };
        let paths: Vec<String> = paths
            .iter()
            .map(|path| path.display().to_string())
            .collect();
        return out
            .emit(
                &CompletionInstallation {
                    shell: shell.to_string(),
                    shortcut: "q",
                    paths: &paths,
                },
                || {
                    let mut text = format!("Installed {shell} shell integration\n");
                    for path in &paths {
                        text.push_str("  ");
                        text.push_str(path);
                        text.push('\n');
                    }
                    text
                },
            )
            .map(|()| 0);
    }
    let script = completion::script(shell)?;
    out.emit(
        &CompletionScript {
            shell: shell.to_string(),
            script: &script,
        },
        || script.clone(),
    )?;
    Ok(0)
}

pub(super) fn manual(out: &Emitter, args: &ManArgs) -> Result<u8> {
    let mut command = Cli::command();
    command.build();
    let Some(directory) = args.dir.as_deref() else {
        let page = render_page(&command, PROGRAM)?;
        let text = String::from_utf8(page).context("the manual page was not valid UTF-8")?;
        return out
            .emit(&ManualPage { page: &text }, || text.clone())
            .map(|()| 0);
    };
    fs::create_dir_all(directory)
        .with_context(|| format!("failed to create {}", directory.display()))?;
    let mut written = Vec::new();
    write_pages(&command, PROGRAM, directory, &mut written)?;
    out.emit(&ManualPages { pages: &written }, || {
        let mut text = format!("Wrote {} pages to {}\n", written.len(), directory.display());
        for page in &written {
            text.push_str("  ");
            text.push_str(page);
            text.push('\n');
        }
        text
    })?;
    Ok(0)
}

pub(super) fn capabilities(out: &Emitter) -> Result<u8> {
    let mut command = Cli::command();
    command.build();
    let mut commands = Vec::new();
    collect_capabilities(&command, &[], &mut commands);
    let document = CapabilityDocument {
        schema_version: 1,
        version: env!("CARGO_PKG_VERSION"),
        output_modes: ["text", "json"],
        commands,
    };
    out.emit(&document, || render_capabilities(&document))?;
    Ok(0)
}

pub(super) fn collect_capabilities(
    command: &clap::Command,
    parent: &[String],
    commands: &mut Vec<CommandCapability>,
) {
    let mut path = parent.to_vec();
    path.push(command.get_name().to_owned());
    let arguments = command
        .get_arguments()
        .filter(|argument| !argument.is_hide_set())
        .filter(|argument| argument.get_id() != "help" && argument.get_id() != "version")
        .map(|argument| {
            let (min_values, max_values) = argument.get_num_args().map_or((0, Some(0)), |range| {
                let maximum = range.max_values();
                (
                    range.min_values(),
                    (maximum != usize::MAX).then_some(maximum),
                )
            });
            ArgumentCapability {
                id: argument.get_id().to_string(),
                help: argument.get_help().map(ToString::to_string),
                short: argument.get_short(),
                long: argument.get_long().map(str::to_owned),
                positional: argument.is_positional(),
                required: argument.is_required_set(),
                action: argument_action(argument.get_action()),
                min_values,
                max_values,
                value_names: argument
                    .get_value_names()
                    .map(|names| names.iter().map(ToString::to_string).collect())
                    .unwrap_or_default(),
                possible_values: argument
                    .get_possible_values()
                    .iter()
                    .map(|value| value.get_name().to_owned())
                    .collect(),
                default_values: argument
                    .get_default_values()
                    .iter()
                    .map(|value| value.to_string_lossy().into_owned())
                    .collect(),
            }
        })
        .collect();
    let usage = command.clone().render_usage().to_string();
    let groups = command
        .get_groups()
        .map(|group| {
            let mut configured = group.clone();
            ArgumentGroupCapability {
                id: configured.get_id().to_string(),
                arguments: configured.get_args().map(ToString::to_string).collect(),
                required: configured.is_required_set(),
                multiple: configured.is_multiple(),
            }
        })
        .collect();
    commands.push(CommandCapability {
        path: path.join(" "),
        about: command.get_about().map(ToString::to_string),
        usage,
        arguments,
        groups,
        subcommands: command
            .get_subcommands()
            .filter(|child| child.get_name() != "help")
            .map(|child| child.get_name().to_owned())
            .collect(),
    });
    for child in command
        .get_subcommands()
        .filter(|child| child.get_name() != "help")
    {
        collect_capabilities(child, &path, commands);
    }
}

pub(super) const fn argument_action(action: &clap::ArgAction) -> &'static str {
    match action {
        clap::ArgAction::Set => "set",
        clap::ArgAction::Append => "append",
        clap::ArgAction::SetTrue => "set_true",
        clap::ArgAction::SetFalse => "set_false",
        clap::ArgAction::Count => "count",
        clap::ArgAction::Help => "help",
        clap::ArgAction::HelpShort => "help_short",
        clap::ArgAction::HelpLong => "help_long",
        clap::ArgAction::Version => "version",
        _ => "other",
    }
}

pub(super) fn render_capabilities(document: &CapabilityDocument) -> String {
    let mut text = format!(
        "Quinjet {} command capabilities (schema {})\n\n",
        document.version, document.schema_version
    );
    for command in &document.commands {
        text.push_str(&command.path);
        if let Some(about) = &command.about {
            text.push_str("  ");
            text.push_str(about);
        }
        text.push('\n');
    }
    text.push_str("\nUse --json for arguments, values, and command relationships.\n");
    text
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CapabilityDocument {
    schema_version: u8,
    version: &'static str,
    output_modes: [&'static str; 2],
    commands: Vec<CommandCapability>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CommandCapability {
    path: String,
    about: Option<String>,
    usage: String,
    arguments: Vec<ArgumentCapability>,
    groups: Vec<ArgumentGroupCapability>,
    subcommands: Vec<String>,
}

#[derive(Serialize)]
pub(super) struct ArgumentGroupCapability {
    id: String,
    arguments: Vec<String>,
    required: bool,
    multiple: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ArgumentCapability {
    id: String,
    help: Option<String>,
    short: Option<char>,
    long: Option<String>,
    positional: bool,
    required: bool,
    action: &'static str,
    min_values: usize,
    max_values: Option<usize>,
    value_names: Vec<String>,
    possible_values: Vec<String>,
    default_values: Vec<String>,
}

pub(super) fn render_page(command: &clap::Command, name: &str) -> Result<Vec<u8>> {
    let mut page = Vec::new();
    Man::new(command.clone().display_name(name.to_owned()))
        .title(name.to_uppercase())
        .render(&mut page)
        .with_context(|| format!("failed to render the manual page for {name}"))?;
    Ok(page)
}

pub(super) fn write_pages(
    command: &clap::Command,
    name: &str,
    directory: &Path,
    written: &mut Vec<String>,
) -> Result<()> {
    let file = directory.join(format!("{name}.1"));
    fs::write(&file, render_page(command, name)?)
        .with_context(|| format!("failed to write {}", file.display()))?;
    written.push(file.display().to_string());
    for child in command
        .get_subcommands()
        .filter(|child| child.get_name() != "help")
    {
        write_pages(
            child,
            &format!("{name}-{}", child.get_name()),
            directory,
            written,
        )?;
    }
    Ok(())
}

#[derive(Serialize)]
pub(super) struct CompletionScript<'a> {
    shell: String,
    script: &'a str,
}

#[derive(Serialize)]
pub(super) struct CompletionInstallation<'a> {
    shell: String,
    shortcut: &'static str,
    paths: &'a [String],
}

#[derive(Serialize)]
pub(super) struct ManualPage<'a> {
    page: &'a str,
}

#[derive(Serialize)]
pub(super) struct ManualPages<'a> {
    pages: &'a [String],
}
