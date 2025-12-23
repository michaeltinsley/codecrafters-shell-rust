use crate::ShellStatus;
use crate::get_executable_path;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::str::FromStr;

/// Enumeration of all supported builtin commands.
pub enum Builtin {
    Exit,
    Echo,
    Type,
    Pwd,
    Cd,
    History,
}

impl FromStr for Builtin {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "exit" => Ok(Builtin::Exit),
            "echo" => Ok(Builtin::Echo),
            "type" => Ok(Builtin::Type),
            "pwd" => Ok(Builtin::Pwd),
            "cd" => Ok(Builtin::Cd),
            "history" => Ok(Builtin::History),
            _ => Err(()),
        }
    }
}

impl Builtin {
    /// Executes the builtin command.
    ///
    /// Returns a `ShellStatus` indicating whether the shell should continue
    /// or exit with a specific code.
    pub fn execute<W: Write, E: Write>(
        &self,
        args: Vec<String>,
        mut stdout: W,
        mut stderr: E,
        history: &[String],
    ) -> ShellStatus {
        match self {
            Builtin::Exit => {
                let code = args
                    .first()
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(0);
                ShellStatus::Exit(code)
            }
            Builtin::Echo => {
                echo_cmd(args, &mut stdout);
                ShellStatus::Continue
            }
            Builtin::Type => {
                type_cmd(args, &mut stdout, &mut stderr);
                ShellStatus::Continue
            }
            Builtin::Pwd => {
                match std::env::current_dir() {
                    Ok(path) => {
                        let _ = writeln!(stdout, "{}", path.display());
                    }
                    Err(e) => {
                        let _ = writeln!(stderr, "pwd: error retrieving current directory: {}", e);
                    }
                }
                ShellStatus::Continue
            }
            Builtin::Cd => {
                if let Some(path) = args.first() {
                    let new_dir = if path == "~" {
                        match std::env::var("HOME") {
                            Ok(val) => val,
                            Err(_) => {
                                let _ = writeln!(stderr, "cd: HOME not set");
                                return ShellStatus::Continue;
                            }
                        }
                    } else {
                        path.clone()
                    };

                    if std::env::set_current_dir(&new_dir).is_err() {
                        let _ = writeln!(stderr, "cd: no such file or directory: {}", new_dir);
                    }
                }
                ShellStatus::Continue
            }
            Builtin::History => {
                // Check for -r flag to read from file
                if args.first().map(|s| s.as_str()) == Some("-r") {
                    if let Some(filepath) = args.get(1) {
                        match File::open(filepath) {
                            Ok(file) => {
                                let reader = BufReader::new(file);
                                let mut loaded_history = Vec::new();
                                for cmd in reader.lines().flatten() {
                                    loaded_history.push(cmd);
                                }
                                return ShellStatus::LoadHistory(loaded_history);
                            }
                            Err(e) => {
                                let _ = writeln!(stderr, "history: {}: {}", filepath, e);
                                return ShellStatus::Continue;
                            }
                        }
                    } else {
                        let _ = writeln!(stderr, "history: -r requires a filename argument");
                        return ShellStatus::Continue;
                    }
                }

                // Parse optional limit argument
                let limit = args.first().and_then(|n_str| n_str.parse::<usize>().ok());

                // Calculate starting index
                let start_idx = if let Some(n) = limit {
                    if history.len() > n {
                        history.len() - n
                    } else {
                        0
                    }
                } else {
                    0
                };

                // Display command history with line numbers
                for (i, cmd) in history[start_idx..].iter().enumerate() {
                    let _ = writeln!(stdout, "{:>5}  {}", start_idx + i + 1, cmd);
                }
                ShellStatus::Continue
            }
        }
    }
}

/// Implementation of the `echo` command.
///
/// Prints the arguments to stdout, separated by spaces.
pub fn echo_cmd<W: Write>(args: Vec<String>, writer: &mut W) {
    let _ = writeln!(writer, "{}", args.join(" "));
}

/// Implementation of the `type` command.
///
/// Identifies whether a command is a builtin or an executable in the PATH.
pub fn type_cmd<W: Write, E: Write>(args: Vec<String>, stdout: &mut W, stderr: &mut E) {
    let command = match args.first() {
        Some(cmd) => cmd,
        None => {
            return;
        }
    };
    // 1. Check if it's a builtin
    if Builtin::from_str(command).is_ok() {
        let _ = writeln!(stdout, "{} is a shell builtin", command);
        return;
    }

    // 2. External command check
    match get_executable_path(command) {
        Some(path) => {
            let _ = writeln!(stdout, "{} is {}", command, path.display());
        }
        None => {
            let _ = writeln!(stderr, "{}: not found", command);
        }
    }
}
