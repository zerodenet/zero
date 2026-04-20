use std::error::Error;
use std::fmt;

pub const DEFAULT_CONFIG_PATH: &str = "examples/v0.1.0/basic.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Run {
        config_path: String,
        status_listen: Option<String>,
    },
    Status {
        config_path: String,
        json: bool,
    },
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    message: String,
}

impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for CliError {}

pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, CliError> {
    let mut args = args.into_iter();
    let _ = args.next();

    let Some(first) = args.next() else {
        return Ok(Command::Run {
            config_path: DEFAULT_CONFIG_PATH.to_owned(),
            status_listen: None,
        });
    };

    match first.as_str() {
        "run" => parse_run(args.collect()),
        "status" => parse_status(args.collect()),
        "help" | "--help" | "-h" => Ok(Command::Help),
        _ if first.starts_with('-') => Err(CliError::new(format!(
            "unknown option `{first}`\n\n{}",
            usage()
        ))),
        _ => {
            let remaining = args.collect::<Vec<_>>();
            if remaining.is_empty() {
                Ok(Command::Run {
                    config_path: first,
                    status_listen: None,
                })
            } else {
                Err(CliError::new(format!(
                    "unexpected extra arguments after config path\n\n{}",
                    usage()
                )))
            }
        }
    }
}

pub fn usage() -> &'static str {
    "Usage:
  zero [CONFIG_PATH]
  zero run [CONFIG_PATH]
  zero run --status-listen HOST:PORT [CONFIG_PATH]
  zero status [--json] [CONFIG_PATH]
  zero help

Examples:
  zero
  zero examples/v0.1.0/basic.json
  zero run --status-listen 127.0.0.1:9090 examples/v0.1.0/basic.json
  zero status --json examples/v0.1.0/basic.json"
}

fn parse_run(args: Vec<String>) -> Result<Command, CliError> {
    match args.as_slice() {
        [] => Ok(Command::Run {
            config_path: DEFAULT_CONFIG_PATH.to_owned(),
            status_listen: None,
        }),
        _ => {
            let mut status_listen = None;
            let mut config_path = None;
            let mut iter = args.into_iter();

            while let Some(arg) = iter.next() {
                match arg.as_str() {
                    "--status-listen" => {
                        let listen = iter.next().ok_or_else(|| {
                            CliError::new(format!(
                                "`run --status-listen` requires a listen address\n\n{}",
                                usage()
                            ))
                        })?;
                        status_listen = Some(listen);
                    }
                    _ if arg.starts_with('-') => {
                        return Err(CliError::new(format!(
                            "unknown run option `{arg}`\n\n{}",
                            usage()
                        )));
                    }
                    _ => {
                        if config_path.is_some() {
                            return Err(CliError::new(format!(
                                "`run` accepts at most one config path\n\n{}",
                                usage()
                            )));
                        }
                        config_path = Some(arg);
                    }
                }
            }

            Ok(Command::Run {
                config_path: config_path.unwrap_or_else(|| DEFAULT_CONFIG_PATH.to_owned()),
                status_listen,
            })
        }
    }
}

fn parse_status(args: Vec<String>) -> Result<Command, CliError> {
    let mut json = false;
    let mut config_path = None;

    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            _ if arg.starts_with('-') => {
                return Err(CliError::new(format!(
                    "unknown status option `{arg}`\n\n{}",
                    usage()
                )))
            }
            _ => {
                if config_path.is_some() {
                    return Err(CliError::new(format!(
                        "`status` accepts at most one config path\n\n{}",
                        usage()
                    )));
                }
                config_path = Some(arg);
            }
        }
    }

    Ok(Command::Status {
        config_path: config_path.unwrap_or_else(|| DEFAULT_CONFIG_PATH.to_owned()),
        json,
    })
}
