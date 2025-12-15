use codecrafters_shell::ShellStatus;
use std::{
    io::{self, Write},
    process,
};

/// Helper function: Handles the messy parts of printing prompts and flushing buffers
fn get_input(prompt: &str) -> io::Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?; // Use '?' to pass errors up, don't crash with unwrap()

    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer)?;

    // Trim whitespace and return a clean String
    Ok(buffer.trim().to_string())
}

fn main() -> io::Result<()> {
    loop {
        let input_string = match get_input("$ ") {
            Ok(input) => input,
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                continue;
            }
        };
        // Handle empty input
        if input_string.is_empty() {
            continue;
        };
        let mut parts = input_string.split_whitespace();
        let command_str = match parts.next() {
            Some(cmd) => cmd,
            None => continue,
        };
        let args: Vec<&str> = parts.collect();

        // Delegate execution logic to the library
        match codecrafters_shell::handle_command(command_str, args) {
            ShellStatus::Exit(code) => process::exit(code),
            ShellStatus::Continue => continue,
        }
    }
}
