use std::error::Error;

use crate::cli::Command;

mod control;
mod inspect;
mod run;
mod tun;

pub async fn execute(command: Command) -> Result<(), Box<dyn Error>> {
    match command {
        command @ (Command::Mode { .. }
        | Command::Select { .. }
        | Command::Flows { .. }
        | Command::Policies { .. }
        | Command::Events { .. }
        | Command::Reload { .. }) => control::execute(command),
        command @ (Command::TunStart { .. }
        | Command::TunStop { .. }
        | Command::TunStatus { .. }) => tun::execute(command),
        command @ Command::Run { .. } => run::execute(command).await,
        command => inspect::execute(command),
    }
}
