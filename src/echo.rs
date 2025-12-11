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
