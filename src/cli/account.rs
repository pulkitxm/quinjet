use anyhow::Result;
use clap::Subcommand;

use super::{Command, Emitter, GitOperation, Session, operate, render};

#[derive(Debug, Subcommand)]
pub(super) enum AccountVerb {
    #[doc = " List saved GitHub accounts and this repository's commit identity"]
    List,
    #[doc = " Pin this repository's commit identity and HTTPS credentials to an account"]
    Switch { login: String },
}

pub(super) fn run(session: &mut Session, out: &Emitter, command: AccountVerb) -> Result<u8> {
    match command {
        AccountVerb::List => {
            let accounts = session.execute(Command::Accounts)?.accounts()?;
            out.emit(&accounts, || render::accounts(&accounts))?;
            Ok(0)
        }
        AccountVerb::Switch { login } => operate(session, out, GitOperation::SwitchAccount(login)),
    }
}
