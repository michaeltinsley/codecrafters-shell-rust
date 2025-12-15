/// Splits an input string into a vector of arguments.
///
/// This tokenizer handles:
/// - Single quotes (`'...'`): Preserves literal contents.
/// - Double quotes (`"..."`): Preserves contents (backslashes not yet handled).
/// - Unquoted text: Split by whitespace.
///
/// # Example
/// ```
/// let args = tokenize("echo 'hello world'");
/// assert_eq!(args, vec!["echo", "hello world"]);
/// ```
pub fn tokenize(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }

        let mut arg = String::new();

        loop {
            match chars.peek() {
                Some('\'') => {
                    chars.next(); // Consume opening '
                    for c in chars.by_ref() {
                        if c == '\'' {
                            break;
                        }
                        arg.push(c);
                    }
                }
                Some('"') => {
                    chars.next(); // Consume opening "
                    for c in chars.by_ref() {
                        if c == '"' {
                            break;
                        }
                        arg.push(c);
                    }
                }
                Some(c) if c.is_whitespace() => break,
                Some(c) => {
                    arg.push(*c);
                    chars.next();
                }
                None => break,
            }
        }
        args.push(arg);
    }
    args
}
