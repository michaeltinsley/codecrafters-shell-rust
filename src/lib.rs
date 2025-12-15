use std::env;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

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
    match command.parse::<Builtin>() {
        Ok(builtin) => builtin.execute(args),
        Err(_) => {
            if get_executable_path(command).is_some() {
                let output = Command::new(command).args(args).spawn();

                match output {
                    Ok(mut child) => {
                        child.wait().unwrap();
                    }
                    Err(e) => println!("{}: error executing command: {}", command, e),
                }
            } else {
                println!("{}: command not found", command);
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
