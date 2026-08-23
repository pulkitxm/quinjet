use std::process::ExitCode;

use crate::cli::TerminalOptions;
use crate::ssh::{self, SshContext};

pub(super) enum Outcome {
    Finished,
    SwitchSshMachine {
        request: ssh::SshSwitch,
        context: Option<SshContext>,
    },
}

pub(super) fn exit_code(options: &TerminalOptions) -> ExitCode {
    let inherited_context = SshContext::from_environment();
    let local_session = inherited_context.is_none();
    let context = inherited_context.or_else(|| crate::cli::local_ssh_context(&options.path));
    match super::open_terminal(options, context.as_ref()) {
        Ok(Outcome::Finished) => ExitCode::SUCCESS,
        Ok(Outcome::SwitchSshMachine {
            request,
            context: updated_context,
        }) if local_session => {
            let Some(mut context) = updated_context.or(context) else {
                return ExitCode::from(crate::cli::EXIT_FAILURE);
            };
            let Some(machine) = context.machines.get(request.index).cloned() else {
                return ExitCode::from(crate::cli::EXIT_FAILURE);
            };
            context.current.clone_from(&machine.target);
            match crate::cli::run_selected_terminal(
                &machine.target,
                &machine.folder,
                context,
                request.mode,
            ) {
                Ok(code) => ExitCode::from(code),
                Err(error) => ExitCode::from(crate::cli::report(&error)),
            }
        }
        Ok(Outcome::SwitchSshMachine { request, context }) => {
            if let Some(context) = context
                && let Err(error) = ssh::emit_handoff_context(&context)
            {
                return ExitCode::from(crate::cli::report(&error));
            }
            ssh::switch_exit_code(request)
                .map_or_else(|| ExitCode::from(crate::cli::EXIT_FAILURE), ExitCode::from)
        }
        Err(error) => ExitCode::from(crate::cli::report(&error)),
    }
}
