use std::fs;
#[allow(unused_imports)]
use std::io::{self, Write};
use std::process::Command;
use std::{path::Path, process};

fn not_found(command: &str) {
    println!("{}: command not found", command);
}

fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current_token = String::new();
    let mut in_single_quotes = false;
    let mut in_double_quotes = false;
    let mut chars = input.chars().peekable();
    let mut escaped = false;

    while let Some(c) = chars.next() {
        match c {
            '\\' if !in_single_quotes => {
                if let Some(&next_char) = chars.peek() {
                    if in_double_quotes {
                        match next_char {
                            '\\' | '$' | '"' | '\n' => {
                                chars.next();
                                current_token.push(next_char);
                            }
                            _ => {
                                current_token.push('\\');
                                current_token.push(next_char);
                                chars.next();
                            }
                        }
                    } else {
                        chars.next();
                        current_token.push(next_char);
                    }
                } else {
                    current_token.push('\\');
                }
            }

            '\'' if !escaped && !in_double_quotes => {
                in_single_quotes = !in_single_quotes;
            }

            '"' if !escaped && !in_single_quotes => {
                in_double_quotes = !in_double_quotes;
            }

            ' ' if !escaped && !in_single_quotes && !in_double_quotes => {
                if !current_token.is_empty() {
                    tokens.push(current_token.clone());
                    current_token.clear();
                }
            }
            _ => {
                current_token.push(c);
            }
        }
        escaped = false;
    }

    if !current_token.is_empty() {
        tokens.push(current_token);
    }

    tokens.into_iter().filter(|s| !s.is_empty()).collect()
}

fn list_directory(path: &str) -> io::Result<()> {
    let path = if path.is_empty() { "." } else { path };
    let entries = fs::read_dir(path)?;

    let mut items: Vec<String> = Vec::new();

    for entry in entries {
        if let Ok(entry) = entry {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type()?.is_dir() {
                items.push(format!("{}/", file_name));
            } else {
                items.push(file_name);
            }
        }
    }

    items.sort();
    println!("{}", items.join(" "));
    Ok(())
}

fn find_in_path(command: &str, path: &str) -> Option<String> {
    let cmd_path = Path::new(command);
    if cmd_path.is_absolute() || command.contains('/') {
        if let Ok(metadata) = fs::metadata(cmd_path) {
            if metadata.is_file() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if metadata.permissions().mode() & 0o111 != 0 {
                        return Some(command.to_string());
                    }
                }
                #[cfg(not(unix))]
                {
                    return Some(command.to_string());
                }
            }
        }
        return None;
    }

    for dir in path.split(':') {
        let full_path = Path::new(dir).join(command);
        if let Ok(metadata) = fs::metadata(&full_path) {
            if metadata.is_file() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if metadata.permissions().mode() & 0o111 != 0 {
                        return Some(full_path.to_string_lossy().to_string());
                    }
                }
                #[cfg(not(unix))]
                {
                    return Some(full_path.to_string_lossy().to_string());
                }
            }
        }
    }
    None
}

fn execute_command(command: &str, args: &[String], env_path: &str) -> io::Result<()> {
    if command == "ls" {
        return list_directory(args.first().map(String::as_str).unwrap_or(""));
    }

    let program = if command.starts_with('\'') || command.starts_with('"') {
        command.to_string()
    } else {
        match find_in_path(command, env_path) {
            Some(path) => path,
            None => {
                not_found(command);
                return Ok(());
            }
        }
    };

    let output = Command::new(program)
        .args(args)
        .output()?;
    
    io::stdout().write_all(&output.stdout)?;
    io::stderr().write_all(&output.stderr)?;
    Ok(())
}

fn expand_tilde(path: &str) -> String {
    if path == "~" {
        std::env::var("HOME").unwrap_or_else(|_| String::from(path))
    } else if path.starts_with("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| String::from("~"));
        path.replacen("~", &home, 1)
    } else {
        String::from(path)
    }
}

fn change_directory(path: &str) -> io::Result<()> {
    let expanded_path = expand_tilde(path);
    match std::env::set_current_dir(&expanded_path) {
        Ok(_) => Ok(()),
        Err(e) => {
            println!("cd: {}: No such file or directory", path);
            Err(e)
        }
    }
}

fn main() {
    let env_path = std::env::var("PATH").unwrap();
    loop {
        print!("$ ");
        if io::stdout().flush().is_err() {
            println!("error while doing the stdout");
            continue;
        }
        let stdin = io::stdin();
        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();
        let command = input.trim();
        let tokens = tokenize(command);

        if tokens.is_empty() {
            continue;
        }

        match tokens.as_slice() {
            [exit_cmd, code] if exit_cmd == "exit" => process::exit(code.parse::<i32>().unwrap()),
            [echo_cmd, args @ ..] if echo_cmd == "echo" => {
                println!("{}", args.join(" "));
            }
            [pwd_cmd] if pwd_cmd == "pwd" => match std::env::current_dir() {
                Ok(path) => println!("{}", path.display()),
                Err(e) => println!("pwd: error getting current directory: {}", e),
            },
            [cd_cmd] if cd_cmd == "cd" => {
                let home = std::env::var("HOME").unwrap_or_default();
                let _ = change_directory(&home);
            }
            [cd_cmd, path] if cd_cmd == "cd" => {
                let _ = change_directory(path);
            }
            [ls_cmd] | [ls_cmd, _] if ls_cmd == "ls" => {
                let path = tokens.get(1).map(String::as_str).unwrap_or("");
                if let Err(e) = list_directory(path) {
                    eprintln!("ls: {}: {}", path, e);
                }
            }
            [type_cmd, ..] if type_cmd == "type" => {
                if tokens.len() != 2 {
                    println!("type: expected 1 argument, got {}", tokens.len() - 1);
                    continue;
                }
                let command = &tokens[1];
                if ["exit", "echo", "type", "pwd", "cd", "ls"].contains(&command.as_str()) {
                    println!("{} is a shell builtin", command);
                    continue;
                }
                match find_in_path(command, &env_path) {
                    Some(path) => println!("{} is {}", command, path),
                    None => println!("{}: not found", command),
                }
            }
            _ => {
                let command = &tokens[0];
                let args = &tokens[1..];
                if let Err(e) = execute_command(command, args, &env_path) {
                    eprintln!("Error executing command: {}", e);
                }
            }
        }
    }
}

