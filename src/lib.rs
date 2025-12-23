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
pub fn handle_command(command: &str, args: Vec<String>, history: &[String]) -> ShellStatus {
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
            builtin.execute(clean_args, &mut *stdout, &mut *stderr, history)
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

/// Executes a pipeline of N commands connected by pipes.
///
/// Takes the full input string, splits it by '|', and executes the commands
/// with each command's stdout connected to the next command's stdin.
/// Supports both built-in and external commands.
pub fn execute_pipeline(input: &str) -> ShellStatus {
    let parts: Vec<&str> = input.split('|').map(|s| s.trim()).collect();

    if parts.is_empty() {
        return ShellStatus::Continue;
    }

    // Parse all commands
    let mut commands: Vec<(String, Vec<String>)> = Vec::new();
    for part in &parts {
        let tokens = tokenize(part);
        if tokens.is_empty() {
            return ShellStatus::Continue;
        }
        let cmd = tokens[0].clone();
        let args = tokens[1..].to_vec();
        commands.push((cmd, args));
    }

    if commands.len() == 1 {
        // Single command, no pipeline needed
        let (cmd, args) = &commands[0];
        return handle_command(cmd, args.clone(), &[]);
    }

    // Create pipes for N-1 connections
    let num_pipes = commands.len() - 1;
    let mut pipes: Vec<(i32, i32)> = Vec::new();

    for _ in 0..num_pipes {
        unsafe {
            let mut fds = [0; 2];
            if libc::pipe(fds.as_mut_ptr()) == -1 {
                eprintln!("Failed to create pipe");
                // Clean up any pipes already created
                for (read_fd, write_fd) in pipes {
                    libc::close(read_fd);
                    libc::close(write_fd);
                }
                return ShellStatus::Continue;
            }
            pipes.push((fds[0], fds[1]));
        }
    }

    // Spawn all commands
    let mut pids: Vec<i32> = Vec::new();

    for (i, (cmd, args)) in commands.iter().enumerate() {
        let is_first = i == 0;
        let is_last = i == commands.len() - 1;
        let is_builtin = Builtin::from_str(cmd).is_ok();

        // Determine stdin for this command
        let stdin_fd = if is_first {
            None
        } else {
            Some(pipes[i - 1].0) // Read from previous pipe
        };

        // Determine stdout for this command
        let stdout_fd = if is_last {
            None
        } else {
            Some(pipes[i].1) // Write to next pipe
        };

        let pid = if is_builtin {
            execute_builtin_in_pipeline(cmd, args.clone(), stdin_fd, stdout_fd)
        } else {
            spawn_external_in_pipeline(cmd, args.clone(), stdin_fd, stdout_fd)
        };

        if pid < 0 {
            eprintln!("Failed to spawn command: {}", cmd);
            // Clean up: kill spawned processes and close pipes
            for spawned_pid in pids {
                unsafe {
                    libc::kill(spawned_pid, libc::SIGKILL);
                }
            }
            for (read_fd, write_fd) in pipes {
                unsafe {
                    libc::close(read_fd);
                    libc::close(write_fd);
                }
            }
            return ShellStatus::Continue;
        }

        pids.push(pid);
    }

    // Close all pipe fds in parent
    for (read_fd, write_fd) in pipes {
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
        }
    }

    // Wait for all processes
    for pid in pids {
        unsafe {
            let mut status: i32 = 0;
            libc::waitpid(pid, &mut status, 0);
        }
    }

    ShellStatus::Continue
}

/// Spawns an external command in a pipeline with redirected I/O.
///
/// Returns the PID of the spawned child process, or -1 on failure.
fn spawn_external_in_pipeline(
    cmd: &str,
    args: Vec<String>,
    stdin_fd: Option<i32>,
    stdout_fd: Option<i32>,
) -> i32 {
    let mut command = Command::new(cmd);
    command.args(&args);

    if let Some(fd) = stdin_fd {
        command.stdin(unsafe { Stdio::from_raw_fd(fd) });
    }

    if let Some(fd) = stdout_fd {
        command.stdout(unsafe { Stdio::from_raw_fd(fd) });
    }

    match command.spawn() {
        Ok(child) => child.id() as i32,
        Err(e) => {
            eprintln!("{}: error executing command: {}", cmd, e);
            -1
        }
    }
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
                match builtin.execute(args, &mut out, &mut err, &[]) {
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
