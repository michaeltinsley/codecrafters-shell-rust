use std::env;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub mod builtins;
pub mod parser;

pub use builtins::Builtin;
pub use parser::tokenize;

/// Result of a command execution.
pub enum ShellStatus {
    /// The shell should continue running.
    Continue,
    /// The shell should exit with the provided code.
    Exit(i32),
}

/// Orchestrates command execution.
///
/// It first attempts to parse the command as a `Builtin`. If that fails,
/// it searches for an external executable in the `PATH` and runs it.
pub fn handle_command(command: &str, args: Vec<String>) -> ShellStatus {
    let mut clean_args = Vec::new();
    let mut stdout_file: Option<File> = None;
    let mut stderr_file: Option<File> = None;
    let mut args_iter = args.into_iter();

    while let Some(arg) = args_iter.next() {
        if arg == ">" || arg == "1>" {
            if let Some(filename) = args_iter.next() {
                stdout_file = Some(File::create(filename).unwrap());
            }
        } else if arg == ">>" || arg == "1>>" {
            if let Some(filename) = args_iter.next() {
                stdout_file = Some(
                    OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(filename)
                        .unwrap(),
                );
            }
        } else if arg == "2>" {
            if let Some(filename) = args_iter.next() {
                stderr_file = Some(File::create(filename).unwrap());
            }
        } else if arg == "2>>" {
            if let Some(filename) = args_iter.next() {
                stderr_file = Some(
                    OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(filename)
                        .unwrap(),
                );
            }
        } else {
            clean_args.push(arg);
        }
    }

    match command.parse::<Builtin>() {
        Ok(builtin) => {
            let mut stdout: Box<dyn std::io::Write> = match stdout_file {
                Some(f) => Box::new(f),
                None => Box::new(std::io::stdout()),
            };
            let mut stderr: Box<dyn std::io::Write> = match stderr_file {
                Some(f) => Box::new(f),
                None => Box::new(std::io::stderr()),
            };
            builtin.execute(clean_args, &mut *stdout, &mut *stderr)
        }
        Err(_) => {
            if get_executable_path(command).is_some() {
                let stdout = match stdout_file {
                    Some(f) => Stdio::from(f),
                    None => Stdio::inherit(),
                };
                let stderr = match stderr_file {
                    Some(f) => Stdio::from(f),
                    None => Stdio::inherit(),
                };

                let output = Command::new(command)
                    .args(clean_args)
                    .stdout(stdout)
                    .stderr(stderr)
                    .spawn();

                match output {
                    Ok(mut child) => {
                        child.wait().unwrap();
                    }
                    Err(e) => eprintln!("{}: error executing command: {}", command, e),
                }
            } else {
                eprintln!("{}: command not found", command);
            }
            ShellStatus::Continue
        }
    }
}

/// Searches the system `PATH` for an executable with the given name.
///
/// Returns `Some(PathBuf)` if found and executable, otherwise `None`.
pub(crate) fn get_executable_path(command: &str) -> Option<PathBuf> {
    let path_var = env::var("PATH").ok()?;

    for path in env::split_paths(&path_var) {
        let full_path = path.join(command);

        if full_path.is_file()
            && let Ok(metadata) = full_path.metadata()
            && metadata.permissions().mode() & 0o111 != 0
        {
            return Some(full_path);
        }
    }
    None
}
