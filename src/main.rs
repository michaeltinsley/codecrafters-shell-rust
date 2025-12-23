use codecrafters_shell::ShellStatus;
use std::{
    io::{self, Write},
    process,
};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;

fn main() -> io::Result<()> {
    loop {
        print!("$ ");
        io::stdout().flush()?;

        // Enter raw mode to handle input character by character
        let mut stdout = io::stdout().into_raw_mode()?;
        let stdin = io::stdin();
        let mut buffer = String::new();

        for c in stdin.keys() {
            match c {
                Ok(key) => match key {
                    Key::Ctrl('c') => {
                        buffer.clear();
                        write!(stdout, "\r\n")?;
                        break;
                    }
                    Key::Ctrl('d') => {
                        if buffer.is_empty() {
                            return Ok(());
                        }
                    }
                    Key::Char('\n') | Key::Char('\r') => {
                        write!(stdout, "\r\n")?;
                        break;
                    }
                    Key::Char('\t') => {
                        let builtins = ["echo", "exit"];
                        let mut matches = builtins.iter().filter(|cmd| cmd.starts_with(&buffer));

                        let first = matches.next();
                        let second = matches.next();

                        if let Some(cmd) = first {
                            if second.is_none() {
                                let remainder = &cmd[buffer.len()..];
                                write!(stdout, "{} ", remainder)?;
                                buffer.push_str(remainder);
                                buffer.push(' ');
                                stdout.flush()?;
                            } else {
                                write!(stdout, "\x07")?;
                                stdout.flush()?;
                            }
                        } else {
                            write!(stdout, "\x07")?;
                            stdout.flush()?;
                        }
                    }
                    Key::Backspace => {
                        if !buffer.is_empty() {
                            buffer.pop();
                            // Move cursor back, erase char with space, move back again
                            write!(stdout, "\x08 \x08")?;
                            stdout.flush()?;
                        }
                    }
                    Key::Char(c) => {
                        buffer.push(c);
                        write!(stdout, "{}", c)?;
                        stdout.flush()?;
                    }
                    _ => {}
                },
                Err(e) => {
                    eprintln!("Error reading input: {}", e);
                    break;
                }
            }
        }

        // Disable raw mode
        drop(stdout);

        let input_string = buffer.trim().to_string();
        if input_string.is_empty() {
            continue;
        }

        let mut parts = codecrafters_shell::tokenize(&input_string).into_iter();
        let command_str = match parts.next() {
            Some(cmd) => cmd,
            None => continue,
        };
        let args: Vec<String> = parts.collect();

        match codecrafters_shell::handle_command(&command_str, args) {
            ShellStatus::Exit(code) => process::exit(code),
            ShellStatus::Continue => continue,
        }
    }
}
