use std::error::Error;
use std::fmt;

pub const DEFAULT_CONFIG_PATH: &str = "examples/v0.0.1/basic.json";

/// Extract the config file path from raw CLI arguments for early
/// initialisation of the tracing subscriber.
pub fn config_path_from_args(args: &[String]) -> String {
    let mut iter = args.iter().skip(1);
    let Some(first) = iter.next() else {
        return DEFAULT_CONFIG_PATH.to_owned();
    };
    match first.as_str() {
        "run" => {
            let mut path = None;
            let mut iter = iter;
            while let Some(arg) = iter.next() {
                match arg.as_str() {
                    "--status-listen" | "--control-socket" | "--ipc-hook-socket" => {
                        iter.next();
                    }
                    a if a.starts_with('-') => {}
                    _ => path = Some(arg.clone()),
                }
            }
            path.unwrap_or_else(|| DEFAULT_CONFIG_PATH.to_owned())
        }
        "status" | "reload" => {
            let mut path = None;
            let mut iter = iter;
            while let Some(arg) = iter.next() {
                match arg.as_str() {
                    "--json" | "--socket" => {
                        if arg == "--socket" {
                            iter.next();
                        }
                    }
                    a if a.starts_with('-') => {}
                    _ => path = Some(arg.clone()),
                }
            }
            path.unwrap_or_else(|| DEFAULT_CONFIG_PATH.to_owned())
        }
        "help" | "--help" | "-h" | "version" | "--version" | "-V" | "select" | "flows" | "policies" | "events" => {
            DEFAULT_CONFIG_PATH.to_owned()
        }
        _ if first.starts_with('-') => DEFAULT_CONFIG_PATH.to_owned(),
        _ => first.clone(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Run {
        config_path: String,
        status_listen: Option<String>,
        control_socket: Option<String>,
        ipc_hook_socket: Option<String>,
    },
    Status {
        config_path: Option<String>,
        json: bool,
        socket_path: Option<String>,
    },
    Select {
        policy_tag: String,
        target_tag: String,
        socket_path: Option<String>,
    },
    Flows {
        socket_path: Option<String>,
    },
    Policies {
        socket_path: Option<String>,
    },
    Events {
        socket_path: Option<String>,
    },
    Reload {
        config_path: String,
        socket_path: Option<String>,
    },
    Version,
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
    let _ = args.next(); // skip program name

    let Some(first) = args.next() else {
        return Ok(Command::Run {
            config_path: DEFAULT_CONFIG_PATH.to_owned(),
            status_listen: None,
            control_socket: None,
            ipc_hook_socket: None,
        });
    };

    match first.as_str() {
        "run" => parse_run(args.collect()),
        "status" => parse_status(args.collect()),
        "select" => parse_select(args.collect()),
        "flows" => parse_client_command(args.collect(), |socket_path| Command::Flows { socket_path }),
        "policies" => {
            parse_client_command(args.collect(), |socket_path| Command::Policies { socket_path })
        }
        "events" => {
            parse_client_command(args.collect(), |socket_path| Command::Events { socket_path })
        }
        "reload" => parse_reload(args.collect()),
        "version" | "--version" | "-V" => Ok(Command::Version),
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
                    control_socket: None,
                    ipc_hook_socket: None,
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
  zero
  zero [CONFIG_PATH]
  zero run [--status-listen HOST:PORT] [--control-socket PATH] [CONFIG_PATH]
  zero status [--json] [--socket PATH] [CONFIG_PATH]
  zero select <policy> <target> [--socket PATH]
  zero flows [--socket PATH]
  zero policies [--socket PATH]
  zero events [--socket PATH]
  zero reload [CONFIG_PATH] [--socket PATH]
  zero version
  zero help

Examples:
  zero
  zero run examples/v0.0.1/basic.json
  zero run --status-listen 127.0.0.1:9090 --control-socket /tmp/zero.sock
  zero select proxy direct
  zero flows
  zero policies
  zero events
  zero reload examples/v0.0.1/basic.json"
}

fn parse_run(args: Vec<String>) -> Result<Command, CliError> {
    let mut status_listen = None;
    let mut control_socket = None;
    let mut ipc_hook_socket = None;
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
            "--control-socket" => {
                let path = iter.next().ok_or_else(|| {
                    CliError::new(format!(
                        "`run --control-socket` requires a socket path\n\n{}",
                        usage()
                    ))
                })?;
                control_socket = Some(path);
            }
            "--ipc-hook-socket" => {
                let path = iter.next().ok_or_else(|| {
                    CliError::new(format!(
                        "`run --ipc-hook-socket` requires a socket path\n\n{}",
                        usage()
                    ))
                })?;
                ipc_hook_socket = Some(path);
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
        control_socket,
        ipc_hook_socket,
    })
}

fn parse_status(args: Vec<String>) -> Result<Command, CliError> {
    let mut json = false;
    let mut config_path = None;
    let mut socket_path = None;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--json" => json = true,
            "--socket" => {
                socket_path = Some(iter.next().ok_or_else(|| {
                    CliError::new("`--socket` requires a path argument")
                })?);
            }
            _ if arg.starts_with('-') => {
                return Err(CliError::new(format!(
                    "unknown status option `{arg}`\n\n{}",
                    usage()
                )));
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
        config_path,
        json,
        socket_path,
    })
}

fn parse_select(args: Vec<String>) -> Result<Command, CliError> {
    let mut iter = args.into_iter();
    let mut positional = Vec::new();
    let mut socket_path = None;

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--socket" => {
                socket_path = Some(iter.next().ok_or_else(|| {
                    CliError::new("`--socket` requires a path argument")
                })?);
            }
            _ if arg.starts_with('-') => {
                return Err(CliError::new(format!(
                    "unknown option `{arg}`\n\n{}",
                    usage()
                )));
            }
            _ => positional.push(arg),
        }
    }

    if positional.len() != 2 {
        return Err(CliError::new(format!(
            "`select` requires <policy> and <target> arguments\n\n{}",
            usage()
        )));
    }

    Ok(Command::Select {
        policy_tag: positional.remove(0),
        target_tag: positional.remove(0),
        socket_path,
    })
}

fn parse_reload(args: Vec<String>) -> Result<Command, CliError> {
    let mut config_path = None;
    let mut socket_path = None;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--socket" => {
                socket_path = Some(iter.next().ok_or_else(|| {
                    CliError::new("`--socket` requires a path argument")
                })?);
            }
            _ if arg.starts_with('-') => {
                return Err(CliError::new(format!(
                    "unknown option `{arg}`\n\n{}",
                    usage()
                )));
            }
            _ => {
                if config_path.is_some() {
                    return Err(CliError::new(format!(
                        "`reload` accepts at most one config path\n\n{}",
                        usage()
                    )));
                }
                config_path = Some(arg);
            }
        }
    }

    Ok(Command::Reload {
        config_path: config_path.unwrap_or_else(|| DEFAULT_CONFIG_PATH.to_owned()),
        socket_path,
    })
}

fn parse_client_command(
    args: Vec<String>,
    make: impl FnOnce(Option<String>) -> Command,
) -> Result<Command, CliError> {
    let mut socket_path = None;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--socket" => {
                socket_path = Some(iter.next().ok_or_else(|| {
                    CliError::new("`--socket` requires a path argument")
                })?);
            }
            _ if arg.starts_with('-') => {
                return Err(CliError::new(format!(
                    "unknown option `{arg}`\n\n{}",
                    usage()
                )));
            }
            _ => {
                return Err(CliError::new(format!(
                    "unexpected argument `{arg}`\n\n{}",
                    usage()
                )));
            }
        }
    }

    Ok(make(socket_path))
}
