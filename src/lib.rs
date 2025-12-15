use std::env;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;

pub enum ShellStatus {
    Continue,
    Exit(i32), // Contains the exit code
}

pub enum Builtin {
    Exit,
    Echo,
    Type,
}

impl FromStr for Builtin {
    type Err = (); // We can just use unit () since we don't need detailed errors yet

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "exit" => Ok(Builtin::Exit),
            "echo" => Ok(Builtin::Echo),
            "type" => Ok(Builtin::Type),
            _ => Err(()),
        }
    }
}

impl Builtin {
    pub fn execute(&self, args: Vec<&str>) -> ShellStatus {
        match self {
            Builtin::Exit => ShellStatus::Exit(0),
            Builtin::Echo => {
                echo_cmd(args);
                ShellStatus::Continue
            }
            Builtin::Type => {
                type_cmd(args);
                ShellStatus::Continue
            }
        }
    }
}

pub fn echo_cmd(args: Vec<&str>) {
    println!("{}", args.join(" "));
}

pub fn type_cmd(args: Vec<&str>) {
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

fn get_executable_path(command: &str) -> Option<PathBuf> {
    // Get the PATH environment variable
    let path_var = env::var("PATH").ok()?;

    // split_paths handles the delimiters for us (: on Unix, ; on Windows)
    for path in env::split_paths(&path_var) {
        let full_path = path.join(command);

        if full_path.is_file()
            && let Ok(metadata) = full_path.metadata()
        {
            let mode = metadata.permissions().mode();
            // 0o111 is the octal mask for execute permissions
            if mode & 0o111 != 0 {
                return Some(full_path);
            }
        }
    }
    None
}

pub fn handle_command(command: &str, args: Vec<&str>) -> ShellStatus {
    match command.parse::<Builtin>() {
        Ok(builtin) => builtin.execute(args),
        Err(_) => {
            // Requirement: Search in PATH (just like 'type' does)
            if let Some(path) = get_executable_path(command) {
                let output = Command::new(path).args(args).spawn();

                match output {
                    Ok(mut child) => {
                        child.wait().unwrap();
                    }
                    Err(e) => println!("{}: error executing command: {}", command, e),
                }
            } else {
                // If get_executable_path returns None, it's not in PATH
                println!("{}: command not found", command);
            }
            ShellStatus::Continue
        }
    }
}
