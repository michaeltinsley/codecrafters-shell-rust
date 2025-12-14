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
    let builtins = ["echo", "type", "exit"];

    if let Some(&first) = args.first() {
        if builtins.contains(&first) {
            println!("{} is a shell builtin", first);
        } else {
            println!("{}: not found", first);
        }
    } else {
        println!("type: missing argument");
    }
}
