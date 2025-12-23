use codecrafters_shell::ShellStatus;
use std::{
    io::{self, Write},
    process,
};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;

/// Calculates the longest common prefix of a list of strings.
fn longest_common_prefix(strings: &[String]) -> String {
    if strings.is_empty() {
        return String::new();
    }

    if strings.len() == 1 {
        return strings[0].clone();
    }

    let first = &strings[0];
    let mut prefix = String::new();

    for (i, ch) in first.chars().enumerate() {
        if strings.iter().all(|s| s.chars().nth(i) == Some(ch)) {
            prefix.push(ch);
        } else {
            break;
        }
    }

    prefix
}

fn main() -> io::Result<()> {
    let mut command_history: Vec<String> = Vec::new();

    loop {
        print!("$ ");
        io::stdout().flush()?;

        // Enter raw mode to handle input character by character
        let mut stdout = io::stdout().into_raw_mode()?;
        let stdin = io::stdin();
        let mut buffer = String::new();
        let mut last_was_tab = false;
        let mut last_tab_matches: Vec<String> = Vec::new();
        let mut last_tab_buffer = String::new();
        let mut history_index: Option<usize> = None;

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
                    Key::Up => {
                        if !command_history.is_empty() {
                            // Navigate backwards in history
                            let new_index = match history_index {
                                None => command_history.len() - 1,
                                Some(0) => 0, // Already at oldest
                                Some(idx) => idx - 1,
                            };

                            history_index = Some(new_index);

                            // Clear current line
                            write!(stdout, "\r$ ")?;
                            for _ in 0..buffer.len() {
                                write!(stdout, " ")?;
                            }
                            write!(stdout, "\r$ ")?;

                            // Load history entry
                            buffer = command_history[new_index].clone();
                            write!(stdout, "{}", buffer)?;
                            stdout.flush()?;
                        }
                        last_was_tab = false;
                    }
                    Key::Down => {
                        if let Some(idx) = history_index {
                            // Navigate forwards in history
                            let new_index = if idx + 1 >= command_history.len() {
                                // At newest, clear buffer
                                history_index = None;

                                // Clear current line
                                write!(stdout, "\r$ ")?;
                                for _ in 0..buffer.len() {
                                    write!(stdout, " ")?;
                                }
                                write!(stdout, "\r$ ")?;

                                buffer.clear();
                                stdout.flush()?;
                                last_was_tab = false;
                                continue;
                            } else {
                                idx + 1
                            };

                            history_index = Some(new_index);

                            // Clear current line
                            write!(stdout, "\r$ ")?;
                            for _ in 0..buffer.len() {
                                write!(stdout, " ")?;
                            }
                            write!(stdout, "\r$ ")?;

                            // Load history entry
                            buffer = command_history[new_index].clone();
                            write!(stdout, "{}", buffer)?;
                            stdout.flush()?;
                        }
                        last_was_tab = false;
                    }
                    Key::Char('\t') => {
                        // Collect all possible completions: builtins and executables
                        let builtins = ["echo", "exit", "type", "pwd", "cd", "history"];
                        let mut all_commands: Vec<String> = builtins
                            .iter()
                            .filter(|cmd| cmd.starts_with(&buffer))
                            .map(|s| s.to_string())
                            .collect();

                        // Add executables from PATH
                        let executables = codecrafters_shell::get_all_executables();
                        all_commands.extend(
                            executables
                                .into_iter()
                                .filter(|cmd| cmd.starts_with(&buffer)),
                        );

                        // Remove duplicates and sort
                        all_commands.sort();
                        all_commands.dedup();

                        if all_commands.len() == 1 {
                            // Single match: complete it with trailing space
                            let cmd = &all_commands[0];
                            let remainder = &cmd[buffer.len()..];
                            write!(stdout, "{} ", remainder)?;
                            buffer.push_str(remainder);
                            buffer.push(' ');
                            stdout.flush()?;
                            last_was_tab = false;
                        } else if all_commands.is_empty() {
                            // No matches: beep
                            write!(stdout, "\x07")?;
                            stdout.flush()?;
                            last_was_tab = false;
                        } else {
                            // Multiple matches: try LCP completion
                            let lcp = longest_common_prefix(&all_commands);

                            if lcp.len() > buffer.len() {
                                // We can complete more - complete to LCP without space
                                let remainder = &lcp[buffer.len()..];
                                write!(stdout, "{}", remainder)?;
                                buffer.push_str(remainder);
                                stdout.flush()?;
                                last_was_tab = false;
                            } else {
                                // LCP equals buffer - can't complete further
                                if last_was_tab
                                    && buffer == last_tab_buffer
                                    && !last_tab_matches.is_empty()
                                {
                                    // Second tab: display all matches
                                    write!(stdout, "\r\n")?;
                                    write!(stdout, "{}\r\n", last_tab_matches.join("  "))?;
                                    write!(stdout, "$ {}", buffer)?;
                                    stdout.flush()?;
                                    last_was_tab = false;
                                } else {
                                    // First tab: beep and store matches
                                    write!(stdout, "\x07")?;
                                    stdout.flush()?;
                                    last_was_tab = true;
                                    last_tab_matches = all_commands;
                                    last_tab_buffer = buffer.clone();
                                }
                            }
                        }
                    }
                    Key::Backspace => {
                        if !buffer.is_empty() {
                            buffer.pop();
                            // Move cursor back, erase char with space, move back again
                            write!(stdout, "\x08 \x08")?;
                            stdout.flush()?;
                        }
                        last_was_tab = false;
                        history_index = None;
                    }
                    Key::Char(c) => {
                        buffer.push(c);
                        write!(stdout, "{}", c)?;
                        stdout.flush()?;
                        last_was_tab = false;
                        history_index = None;
                    }
                    _ => {
                        last_was_tab = false;
                    }
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

        // Add to history
        command_history.push(input_string.clone());

        // Check if this is a pipeline command
        if input_string.contains('|') {
            match codecrafters_shell::execute_pipeline(&input_string) {
                ShellStatus::Exit(code) => process::exit(code),
                ShellStatus::Continue => continue,
            }
        }

        let mut parts = codecrafters_shell::tokenize(&input_string).into_iter();
        let command_str = match parts.next() {
            Some(cmd) => cmd,
            None => continue,
        };
        let args: Vec<String> = parts.collect();

        match codecrafters_shell::handle_command(&command_str, args, &command_history) {
            ShellStatus::Exit(code) => process::exit(code),
            ShellStatus::Continue => continue,
        }
    }
}
