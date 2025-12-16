/// Splits an input string into a vector of arguments.
///
/// This tokenizer handles:
/// - Single quotes (`'...'`): Preserves literal contents.
/// - Double quotes (`"..."`): Preserves contents, handling backslash escapes.
/// - Unquoted text: Split by whitespace, handling backslash escapes.
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
                    while let Some(&c) = chars.peek() {
                        if c == '"' {
                            chars.next();
                            break;
                        }
                        if c == '\\' {
                            chars.next(); // Consume \
                            match chars.peek() {
                                Some(&next_c)
                                    if next_c == '\\'
                                        || next_c == '$'
                                        || next_c == '"'
                                        || next_c == '\n' =>
                                {
                                    arg.push(next_c);
                                    chars.next();
                                }
                                _ => {
                                    arg.push('\\');
                                }
                            }
                        } else {
                            arg.push(c);
                            chars.next();
                        }
                    }
                }
                Some('\\') => {
                    chars.next(); // Consume \
                    if let Some(c) = chars.next() {
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
