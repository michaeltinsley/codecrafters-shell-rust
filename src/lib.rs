use std::env;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::FromRawFd;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::str::FromStr;

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
/// Supports both built-in and external commands.
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
    let args1: Vec<String> = cmd1_tokens[1..].to_vec();

    let cmd2 = &cmd2_tokens[0];
    let args2: Vec<String> = cmd2_tokens[1..].to_vec();

    // Check if commands are built-ins
    let cmd1_is_builtin = Builtin::from_str(cmd1).is_ok();
    let cmd2_is_builtin = Builtin::from_str(cmd2).is_ok();

    // Create a pipe
    let (pipe_read_fd, pipe_write_fd) = unsafe {
        let mut fds = [0; 2];
        if libc::pipe(fds.as_mut_ptr()) == -1 {
            eprintln!("Failed to create pipe");
            return ShellStatus::Continue;
        }
        (fds[0], fds[1])
    };

    // Spawn/execute first command
    let pid1 = if cmd1_is_builtin {
        execute_builtin_in_pipeline(cmd1, args1, None, Some(pipe_write_fd))
    } else {
        match Command::new(cmd1)
            .args(&args1)
            .stdout(unsafe { Stdio::from_raw_fd(pipe_write_fd) })
            .spawn()
        {
            Ok(child) => {
                // Close pipe_write_fd in parent since child now owns it
                child.id() as i32
            }
            Err(e) => {
                eprintln!("{}: error executing command: {}", cmd1, e);
                unsafe {
                    libc::close(pipe_read_fd);
                    libc::close(pipe_write_fd);
                }
                return ShellStatus::Continue;
            }
        }
    };

    // Spawn/execute second command
    let pid2 = if cmd2_is_builtin {
        execute_builtin_in_pipeline(cmd2, args2, Some(pipe_read_fd), None)
    } else {
        match Command::new(cmd2)
            .args(&args2)
            .stdin(unsafe { Stdio::from_raw_fd(pipe_read_fd) })
            .spawn()
        {
            Ok(child) => {
                // Close pipe_read_fd in parent since child now owns it
                child.id() as i32
            }
            Err(e) => {
                eprintln!("{}: error executing command: {}", cmd2, e);
                unsafe {
                    libc::close(pipe_read_fd);
                }
                return ShellStatus::Continue;
            }
        }
    };

    // Close pipe fds in parent if not already closed
    if !cmd1_is_builtin {
        unsafe { libc::close(pipe_write_fd) };
    }
    if !cmd2_is_builtin {
        unsafe { libc::close(pipe_read_fd) };
    }

    // Wait for both processes
    unsafe {
        let mut status: i32 = 0;
        libc::waitpid(pid1, &mut status, 0);
        libc::waitpid(pid2, &mut status, 0);
    }

    ShellStatus::Continue
}

/// Executes a built-in command in a forked child process with redirected I/O.
///
/// Returns the PID of the forked child process.
fn execute_builtin_in_pipeline(
    cmd: &str,
    args: Vec<String>,
    stdin_fd: Option<i32>,
    stdout_fd: Option<i32>,
) -> i32 {
    unsafe {
        let pid = libc::fork();

        if pid == 0 {
            // Child process

            // Redirect stdin if needed
            if let Some(fd) = stdin_fd {
                libc::dup2(fd, 0); // stdin
                libc::close(fd);
            }

            // Redirect stdout if needed
            if let Some(fd) = stdout_fd {
                libc::dup2(fd, 1); // stdout
                libc::close(fd);
            }

            // Execute the built-in
            if let Ok(builtin) = Builtin::from_str(cmd) {
                use std::io::{stderr, stdout};
                let mut out = stdout();
                let mut err = stderr();
                match builtin.execute(args, &mut out, &mut err) {
                    ShellStatus::Exit(code) => std::process::exit(code),
                    ShellStatus::Continue => std::process::exit(0),
                }
            }

            std::process::exit(1);
        } else if pid > 0 {
            // Parent process
            // Close the fds we passed to child
            if let Some(fd) = stdin_fd {
                libc::close(fd);
            }
            if let Some(fd) = stdout_fd {
                libc::close(fd);
            }
            pid
        } else {
            // Fork failed
            eprintln!("Failed to fork for built-in command");
            if let Some(fd) = stdin_fd {
                libc::close(fd);
            }
            if let Some(fd) = stdout_fd {
                libc::close(fd);
            }
            -1
        }
    }
}
