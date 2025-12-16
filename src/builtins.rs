use crate::ShellStatus;
use crate::get_executable_path;
use std::str::FromStr;

/// Enumeration of all supported builtin commands.
pub enum Builtin {
    Exit,
    Echo,
    Type,
    Pwd,
    Cd,
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
            _ => Err(()),
        }
    }
}

impl Builtin {
    /// Executes the builtin command.
    ///
    /// Returns a `ShellStatus` indicating whether the shell should continue
    /// or exit with a specific code.
    pub fn execute(&self, args: Vec<String>) -> ShellStatus {
        match self {
            Builtin::Exit => {
                let code = args
                    .first()
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(0);
                ShellStatus::Exit(code)
            }
            Builtin::Echo => {
                echo_cmd(args);
                ShellStatus::Continue
            }
            Builtin::Type => {
                type_cmd(args);
                ShellStatus::Continue
            }
            Builtin::Pwd => {
                match std::env::current_dir() {
                    Ok(path) => {
                        println!("{}", path.display());
                    }
                    Err(e) => {
                        eprintln!("pwd: error retrieving current directory: {}", e);
                    }
                }
                ShellStatus::Continue
            }
            Builtin::Cd => {
                if let Some(path) = args.first()
                    && std::env::set_current_dir(path).is_err()
                {
                    println!("cd: {}: No such file or directory", path);
                }
                ShellStatus::Continue
            }
        }
    }
}

/// Implementation of the `echo` command.
///
/// Prints the arguments to stdout, separated by spaces.
pub fn echo_cmd(args: Vec<String>) {
    println!("{}", args.join(" "));
}

/// Implementation of the `type` command.
///
/// Identifies whether a command is a builtin or an executable in the PATH.
pub fn type_cmd(args: Vec<String>) {
    let command = match args.first() {
        Some(cmd) => cmd,
        None => {
            return;
        }
    };
    // 1. Check if it's a builtin
    if Builtin::from_str(command).is_ok() {
        println!("{} is a shell builtin", command);
        return;
    }

    // 2. External command check
    match get_executable_path(command) {
        Some(path) => println!("{} is {}", command, path.display()),
        None => println!("{}: not found", command),
    }
}
