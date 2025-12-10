#[allow(unused_imports)]
use std::io::{self, Write};

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
        let command = match get_input("$ ") {
            Ok(input) => input,
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                continue;
            }
        };
        // 2. Handle empty input (user just hit Enter)
        if command.is_empty() {
            continue;
        };
        match command.as_str() {
            "exit" => break,
            "hi" => println!("Hello there!"), // Easy to add new commands
            unknown => println!("{}: command not found", unknown),
        }
    }
    Ok(())
}
