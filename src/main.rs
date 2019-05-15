use std::env;

// yo! here is what I'm referencing:
// https://doc.rust-lang.org/book/ch12-01-accepting-command-line-arguments.html

fn main() {
    let args: Vec<String> = env::args().collect();

    let command = &args[1];
    let org = &args[2];
    let user = &args[3];
    println!("Command: {}", command);
    println!("Organization: {}", org);
    println!("User: {}", user);
    println!("Args: {:#?}", args);
}