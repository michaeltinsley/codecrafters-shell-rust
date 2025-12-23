use std::env;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::FromRawFd;
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
        match arg.as_str() {
            ">" | "1>" => {
                if let Some(filename) = args_iter.next() {
                    stdout_file = Some(File::create(filename).unwrap());
                }
            }
            ">>" | "1>>" => {
                if let Some(filename) = args_iter.next() {
                    stdout_file = Some(
                        OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(filename)
                            .unwrap(),
                    );
                }
            }
            "2>" => {
                if let Some(filename) = args_iter.next() {
                    stderr_file = Some(File::create(filename).unwrap());
                }
            }
            "2>>" => {
                if let Some(filename) = args_iter.next() {
                    stderr_file = Some(
                        OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(filename)
                            .unwrap(),
                    );
                }
            }
            _ => clean_args.push(arg),
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

/// Gets all executable names from directories in the system `PATH`.
///
/// Returns a vector of executable names (not full paths).
/// Handles non-existent directories gracefully.
pub fn get_all_executables() -> Vec<String> {
    let mut executables = Vec::new();

    if let Ok(path_var) = env::var("PATH") {
        for path in env::split_paths(&path_var) {
            if let Ok(entries) = std::fs::read_dir(&path) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata()
                        && metadata.is_file()
                        && metadata.permissions().mode() & 0o111 != 0
                        && let Some(name) = entry.file_name().to_str()
                    {
                        executables.push(name.to_string());
                    }
                }
            }
        }
    }

    executables
}

/// Executes a pipeline of two commands connected by a pipe.
///
/// Takes the full input string, splits it by '|', and executes the two commands
/// with the first command's stdout connected to the second command's stdin.
pub fn execute_pipeline(input: &str) -> ShellStatus {
    let parts: Vec<&str> = input.split('|').map(|s| s.trim()).collect();

    if parts.len() != 2 {
        eprintln!("Pipeline execution only supports exactly 2 commands");
        return ShellStatus::Continue;
    }

    let cmd1_tokens = tokenize(parts[0]);
    let cmd2_tokens = tokenize(parts[1]);

    if cmd1_tokens.is_empty() || cmd2_tokens.is_empty() {
        return ShellStatus::Continue;
    }

    let cmd1 = &cmd1_tokens[0];
    let args1 = &cmd1_tokens[1..];

    let cmd2 = &cmd2_tokens[0];
    let args2 = &cmd2_tokens[1..];

    // Create a pipe
    let (pipe_read_fd, pipe_write_fd) = unsafe {
        let mut fds = [0; 2];
        if libc::pipe(fds.as_mut_ptr()) == -1 {
            eprintln!("Failed to create pipe");
            return ShellStatus::Continue;
        }
        (fds[0], fds[1])
    };

    // Spawn first command with stdout redirected to pipe write end
    let mut child1 = match Command::new(cmd1)
        .args(args1)
        .stdout(unsafe { Stdio::from_raw_fd(pipe_write_fd) })
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            eprintln!("{}: error executing command: {}", cmd1, e);
            unsafe {
                libc::close(pipe_read_fd);
                libc::close(pipe_write_fd);
            }
            return ShellStatus::Continue;
        }
    };

    // Spawn second command with stdin redirected from pipe read end
    let mut child2 = match Command::new(cmd2)
        .args(args2)
        .stdin(unsafe { Stdio::from_raw_fd(pipe_read_fd) })
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            eprintln!("{}: error executing command: {}", cmd2, e);
            unsafe {
                libc::close(pipe_read_fd);
            }
            // Kill first child if second fails to spawn
            let _ = child1.kill();
            let _ = child1.wait();
            return ShellStatus::Continue;
        }
    };

    // Wait for both commands to complete
    let _ = child1.wait();
    let _ = child2.wait();

    ShellStatus::Continue
}
