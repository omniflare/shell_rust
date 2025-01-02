use std::fs;
use std::fs::OpenOptions;
#[allow(unused_imports)]
use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::{path::Path, process};

fn not_found(command: &str) {
    println!("{}: command not found", command);
}

#[derive(Debug, Clone)]
enum Redirection {
    None,
    OutputTo(String),
    OutputAppend(String),
    ErrorTo(String),
    ErrorAppend(String),
    Pipe,
}

#[derive(Debug)]
struct PipelineCommand {
    command: String,
    args: Vec<String>,
    redirection: Redirection,
}

fn parse_redirection(tokens: &[String]) -> (Vec<String>, Redirection) {
    let mut command_parts = Vec::new();
    let mut redirection = Redirection::None;
    let mut i = 0;

    while i < tokens.len() {
        match tokens[i].as_str() {
            ">" | "1>" => {
                if i + 1 < tokens.len() {
                    redirection = Redirection::OutputTo(tokens[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            ">>" | "1>>" => {
                if i + 1 < tokens.len() {
                    redirection = Redirection::OutputAppend(tokens[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "2>" => {
                if i + 1 < tokens.len() {
                    redirection = Redirection::ErrorTo(tokens[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "2>>" => {
                if i + 1 < tokens.len() {
                    redirection = Redirection::ErrorAppend(tokens[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "|" => {
                redirection = Redirection::Pipe;
                i += 1;
            }
            _ => {
                command_parts.push(tokens[i].clone());
                i += 1;
            }
        }
    }

    (command_parts, redirection)
}

// fn setup_redirection(
//     redirection: &Redirection,
//     stdout_pipe: Option<Stdio>,
// ) -> io::Result<(Option<Stdio>, Option<Stdio>)> {
//     let stdout = match redirection {
//         Redirection::OutputTo(path) => Some(Stdio::from(
//             OpenOptions::new()
//                 .write(true)
//                 .create(true)
//                 .truncate(true)
//                 .open(path)?,
//         )),
//         Redirection::OutputAppend(path) => Some(Stdio::from(
//             OpenOptions::new()
//                 .write(true)
//                 .create(true)
//                 .append(true)
//                 .open(path)?,
//         )),
//         Redirection::Pipe => stdout_pipe,
//         _ => None,
//     };

//     let stderr = match redirection {
//         Redirection::ErrorTo(path) => Some(Stdio::from(
//             OpenOptions::new()
//                 .write(true)
//                 .create(true)
//                 .truncate(true)
//                 .open(path)?,
//         )),
//         Redirection::ErrorAppend(path) => Some(Stdio::from(
//             OpenOptions::new()
//                 .write(true)
//                 .create(true)
//                 .append(true)
//                 .open(path)?,
//         )),
//         _ => None,
//     };

//     Ok((stdout, stderr))
// }

fn parse_pipeline(tokens: &[String]) -> Vec<PipelineCommand> {
    let mut pipeline = Vec::new();
    let mut current_command = Vec::new();

    for token in tokens {
        if token == "|" {
            if !current_command.is_empty() {
                let (command_parts, _) = parse_redirection(&current_command);
                if !command_parts.is_empty() {
                    pipeline.push(PipelineCommand {
                        command: command_parts[0].clone(),
                        args: command_parts[1..].to_vec(),
                        redirection: Redirection::Pipe,
                    });
                }
                current_command.clear();
            }
        } else {
            current_command.push(token.clone());
        }
    }

    if !current_command.is_empty() {
        let (command_parts, redirection) = parse_redirection(&current_command);
        if !command_parts.is_empty() {
            pipeline.push(PipelineCommand {
                command: command_parts[0].clone(),
                args: command_parts[1..].to_vec(),
                redirection,
            });
        }
    }

    pipeline
}

fn execute_command(
    command: &str,
    args: &[String],
    env_path: &str,
    redirection: Redirection,
    stdin: Option<Stdio>,
) -> io::Result<Option<Stdio>> {
    let program = if command.starts_with('\'') || command.starts_with('"') {
        command.to_string()
    } else {
        match find_in_path(command, env_path) {
            Some(path) => path,
            None => {
                not_found(command);
                return Ok(None);
            }
        }
    };

    let mut cmd = Command::new(&program);
    cmd.args(args);

    // Set up stdin if provided
    if let Some(stdin) = stdin {
        cmd.stdin(stdin);
    }

    match &redirection {
        Redirection::Pipe => {
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::inherit());
            let child = cmd.spawn()?;
            Ok(child.stdout.map(Stdio::from))
        }
        Redirection::ErrorTo(path) | Redirection::ErrorAppend(path) => {
            cmd.stdout(Stdio::inherit());
            cmd.stderr(Stdio::piped());
            let output = cmd.output()?;

            let stderr_str = String::from_utf8_lossy(&output.stderr);
            let cleaned_stderr = stderr_str.replace(&format!("/usr/bin/{}", command), command);

            let mut file = if matches!(redirection, Redirection::ErrorTo(_)) {
                OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(path)?
            } else {
                OpenOptions::new()
                    .write(true)
                    .create(true)
                    .append(true)
                    .open(path)?
            };
            file.write_all(cleaned_stderr.as_bytes())?;
            Ok(None)
        }
        Redirection::OutputTo(path) | Redirection::OutputAppend(path) => {
            cmd.stderr(Stdio::inherit());
            cmd.stdout(Stdio::piped());
            let output = cmd.output()?;

            let mut file = if matches!(redirection, Redirection::OutputTo(_)) {
                OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(path)?
            } else {
                OpenOptions::new()
                    .write(true)
                    .create(true)
                    .append(true)
                    .open(path)?
            };
            file.write_all(&output.stdout)?;
            Ok(None)
        }
        Redirection::None => {
            cmd.stdout(Stdio::inherit());
            cmd.stderr(Stdio::inherit());
            let status = cmd.status()?;
            if !status.success() {
                return Ok(None);
            }
            Ok(None)
        }
    }
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

        let pipeline = parse_pipeline(&tokens);
        if pipeline.is_empty() {
            continue;
        }
        
        if pipeline.len() == 1 {
            let cmd = &pipeline[0];
            match cmd.command.as_str() {
                "exit" if !cmd.args.is_empty() => {
                    process::exit(cmd.args[0].parse::<i32>().unwrap_or(0))
                }
                "cd" => {
                    let path = cmd.args.first().map(String::as_str).unwrap_or("");
                    let _ = if path.is_empty() {
                        let home = std::env::var("HOME").unwrap_or_default();
                        change_directory(&home)
                    } else {
                        change_directory(path)
                    };
                    continue;
                }
                "pwd" => {
                    if let Ok(path) = std::env::current_dir() {
                        println!("{}", path.display());
                    }
                    continue;
                }
                _ => {}
            }
        }

        let mut previous_output = None;
        for (i, cmd) in pipeline.iter().enumerate() {
            let is_last = i == pipeline.len() - 1;
            let redirection = if is_last {
                cmd.redirection.clone()
            } else {
                Redirection::Pipe
            };

            match execute_command(&cmd.command, &cmd.args, &env_path, redirection, previous_output) {
                Ok(output) => {
                    previous_output = output;
                }
                Err(e) => {
                    eprintln!("Error executing command: {}", e);
                    break;
                }
            }
        }
    }
}