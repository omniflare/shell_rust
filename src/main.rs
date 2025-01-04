use std::collections::HashMap;
use std::fs::{self, File};
use std::fs::OpenOptions;
use std::io::{self, Write, Read};
use std::process::{Command, Stdio};
use std::{path::Path, process};
use std::env;

#[derive(Debug, PartialEq, Clone)]
enum TokenType {
    Word(String),
    Pipe,
    Redirect(RedirectType),
    And,
    Or,  
    Semicolon,
    Quote(String, bool),
}

#[derive(Debug, PartialEq, Clone)]
enum RedirectType {
    Output,
    Append,
    Error,
    ErrorAppend,
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

struct Lexer {
    input: Vec<char>,
    position: usize,
    env_vars: HashMap<String, String>,
}

const HISTORY_FILE_NAME: &str = ".rush_history";
const MAX_HISTORY: usize = 1000;

#[derive(Debug)]
struct History {
    entries: Vec<String>,
    position: isize,
    history_file_path: String,
}

impl Iterator for Lexer {
    type Item = TokenType;
    fn next(&mut self) -> Option<Self::Item> {
        self.next_token()
    }
}

impl Lexer {
    fn new(input: &str, env_vars: HashMap<String, String>) -> Self {
        Lexer {
            input: input.chars().collect(),
            position: 0,
            env_vars,
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.position).copied()
    }

    fn advance(&mut self) -> Option<char> {
        if self.position < self.input.len() {
            let current = self.input[self.position];
            self.position += 1;
            Some(current)
        } else {
            None
        }
    }

    fn lex_quote(&mut self, quote_char: char) -> Option<TokenType> {
        let mut content = String::new();
        let is_single = quote_char == '\'';

        while let Some(c) = self.advance() {
            if c == quote_char {
                return Some(TokenType::Quote(content, is_single));
            }
            if c == '\\' && !is_single {
                if let Some(next) = self.advance() {
                    content.push(next);
                }
            } else if c == '$' && !is_single {
                if let Some(var) = self.lex_variable() {
                    content.push_str(&var);
                }
            } else {
                content.push(c);
            }
        }
        None
    }

    fn lex_variable(&mut self) -> Option<String> {
        let mut var_name = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                var_name.push(c);
                self.advance();
            } else {
                break;
            }
        }
        self.env_vars.get(&var_name).cloned()
    }

    fn lex_redirect(&mut self) -> TokenType {
        match self.peek() {
            Some('>') => {
                self.advance();
                TokenType::Redirect(RedirectType::Append)
            }
            Some('2') if self.input.get(self.position + 1) == Some(&'>') => {
                self.advance();
                self.advance();
                if self.peek() == Some('>') {
                    self.advance();
                    TokenType::Redirect(RedirectType::ErrorAppend)
                } else {
                    TokenType::Redirect(RedirectType::Error)
                }
            }
            _ => TokenType::Redirect(RedirectType::Output),
        }
    }

    fn next_token(&mut self) -> Option<TokenType> {
        while let Some(c) = self.advance() {
            match c {
                ' ' | '\t' | '\n' => continue,
                '|' => {
                    if self.peek() == Some('|') {
                        self.advance();
                        return Some(TokenType::Or);
                    }
                    return Some(TokenType::Pipe);
                }
                '>' => return Some(self.lex_redirect()),
                ';' => return Some(TokenType::Semicolon),
                '\'' | '"' => return self.lex_quote(c),
                '$' => {
                    if let Some(var) = self.lex_variable() {
                        return Some(TokenType::Word(var));
                    }
                }
                '&' if self.peek() == Some('&') => {
                    self.advance();
                    return Some(TokenType::And);
                }
                _ => {
                    let mut word = String::from(c);
                    while let Some(next) = self.peek() {
                        if next.is_whitespace() || ")|><;&".contains(next) {
                            break;
                        }
                        word.push(next);
                        self.advance();
                    }
                    return Some(TokenType::Word(word));
                }
            }
        }
        None
    }
}

impl History {
    fn new() -> Self {
        let home_dir = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let history_file_path = format!("{}/{}", home_dir, HISTORY_FILE_NAME);
        
        let mut history = History {
            entries: Vec::new(),
            position: -1,
            history_file_path,
        };
        history.load_from_file();
        history
    }

    fn load_from_file(&mut self) {
        if let Ok(mut file) = File::open(&self.history_file_path) {
            let mut contents = String::new();
            if file.read_to_string(&mut contents).is_ok() {
                self.entries = contents.lines()
                    .map(String::from)
                    .collect::<Vec<_>>();
                self.position = self.entries.len() as isize;
            }
        }
    }

    fn save_to_file(&self) {
        if let Ok(mut file) = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.history_file_path)
        {
            for entry in &self.entries {
                writeln!(file, "{}", entry).ok();
            }
        }
    }

    fn add(&mut self, command: String) {
        if !command.trim().is_empty() && self.entries.last().map_or(true, |last| last != &command) {
            self.entries.push(command);
            if self.entries.len() > MAX_HISTORY {
                self.entries.remove(0);
            }
            self.position = self.entries.len() as isize;
            self.save_to_file();
        }
    }

    fn get_previous(&mut self) -> Option<&String> {
        if self.position > 0 && !self.entries.is_empty() {
            self.position -= 1;
            self.entries.get(self.position as usize)
        } else {
            self.entries.get(0)
        }
    }

    fn get_next(&mut self) -> Option<&String> {
        if (self.position + 1) < self.entries.len() as isize {
            self.position += 1;
            self.entries.get(self.position as usize)
        } else {
            self.position = self.entries.len() as isize;
            None
        }
    }

    fn reset_position(&mut self) {
        self.position = self.entries.len() as isize;
    }
}


// helper functions

fn not_found(command: &str) {
    println!("{}: command not found", command);
}

fn execute_command(
    command: &str,
    args: &[String],
    env_path: &str,
    redirection: Redirection,
    stdin: Option<Stdio>,
) -> io::Result<(Option<Stdio>, bool)> {
    let program = if command.starts_with('\'') || command.starts_with('"') {
        command.to_string()
    } else {
        match find_in_path(command, env_path) {
            Some(path) => path,
            None => {
                not_found(command);
                return Ok((None, false));
            }
        }
    };

    let mut cmd = Command::new(&program);
    cmd.args(args);

    if let Some(stdin) = stdin {
        cmd.stdin(stdin);
    }

    match &redirection {
        Redirection::Pipe => {
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::inherit());
            let mut child = cmd.spawn()?;
            let success = child.wait()?.success();
            Ok((child.stdout.map(Stdio::from), success))
        }
        Redirection::ErrorTo(path) | Redirection::ErrorAppend(path) => {
            cmd.stdout(Stdio::inherit());
            cmd.stderr(Stdio::piped());
            let output = cmd.output()?;
            let success = output.status.success();

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
            Ok((None, success))
        }
        Redirection::OutputTo(path) | Redirection::OutputAppend(path) => {
            cmd.stderr(Stdio::inherit());
            cmd.stdout(Stdio::piped());
            let output = cmd.output()?;
            let success = output.status.success();

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
            Ok((None, success))
        }
        Redirection::None => {
            cmd.stdout(Stdio::inherit());
            cmd.stderr(Stdio::inherit());
            let status = cmd.status()?;
            Ok((None, status.success()))
        }
    }
}

fn parse_command(tokens: &[TokenType]) -> Option<PipelineCommand> {
    let mut command = None;
    let mut args = Vec::new();
    let mut redirection = Redirection::None;
    let mut i = 0;

    while i < tokens.len() {
        match &tokens[i] {
            TokenType::Word(word) | TokenType::Quote(word, _) => {
                if command.is_none() {
                    command = Some(word.clone());
                } else {
                    args.push(word.clone());
                }
                i += 1;
            }
            TokenType::Redirect(redir_type) => {
                if i + 1 < tokens.len() {
                    if let TokenType::Word(path) | TokenType::Quote(path, _) = &tokens[i + 1] {
                        redirection = match redir_type {
                            RedirectType::Output => Redirection::OutputTo(path.clone()),
                            RedirectType::Append => Redirection::OutputAppend(path.clone()),
                            RedirectType::Error => Redirection::ErrorTo(path.clone()),
                            RedirectType::ErrorAppend => Redirection::ErrorAppend(path.clone()),
                        };
                        i += 2;
                    } else {
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
            TokenType::Pipe => {
                redirection = Redirection::Pipe;
                i += 1;
            }
            _ => i += 1,
        }
    }

    command.map(|cmd| PipelineCommand {
        command: cmd,
        args,
        redirection,
    })
}

fn parse_pipeline(tokens: Vec<TokenType>) -> Vec<(Vec<PipelineCommand>, Option<TokenType>)> {
    let mut pipelines = Vec::new();
    let mut current_pipeline = Vec::new();
    let mut current_tokens = Vec::new();

    for token in tokens.iter() {
        match token {
            TokenType::Pipe => {
                if !current_tokens.is_empty() {
                    if let Some(command) = parse_command(&current_tokens) {
                        current_pipeline.push(command);
                    }
                    current_tokens.clear();
                }
            }
            TokenType::And | TokenType::Or | TokenType::Semicolon => {
                if !current_tokens.is_empty() {
                    if let Some(command) = parse_command(&current_tokens) {
                        current_pipeline.push(command);
                    }
                    current_tokens.clear();
                }
                if !current_pipeline.is_empty() {
                    pipelines.push((current_pipeline, Some(token.clone())));
                    current_pipeline = Vec::new();
                }
            }
            _ => current_tokens.push(token.clone()),
        }
    }

    if !current_tokens.is_empty() {
        if let Some(command) = parse_command(&current_tokens) {
            current_pipeline.push(command);
        }
    }
    if !current_pipeline.is_empty() {
        pipelines.push((current_pipeline, None));
    }

    pipelines
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
    let mut env_vars = HashMap::new();
    env_vars.insert("HOME".to_string(), std::env::var("HOME").unwrap_or_default());
    env_vars.insert("PATH".to_string(), env_path.clone());
    
    let mut history = History::new();
    
    loop {
        print!("$ ");
        if io::stdout().flush().is_err() {
            println!("Error flushing stdout");
            continue;
        }

        let mut current_input = String::new();
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        let mut chars = stdin.bytes();

        'input: loop {
            match chars.next() {
                Some(Ok(b'\n')) => {
                    println!();
                    break;
                }
                Some(Ok(3)) => { // Ctrl-C
                    println!("^C");
                    current_input.clear();
                    break;
                }
                Some(Ok(4)) => { // Ctrl-D
                    if current_input.is_empty() {
                        println!();
                        process::exit(0);
                    }
                }
                Some(Ok(27)) => {
                    if let (Some(Ok(91)), Some(Ok(code))) = (chars.next(), chars.next()) {
                        match code {
                            65 => {
                                if let Some(previous) = history.get_previous() {
                                    print!("\r$ {}", " ".repeat(current_input.len()));
                                    print!("\r$ {}", previous);
                                    stdout.flush().unwrap();
                                    current_input = previous.clone();
                                }
                            }
                            66 => { 
                                print!("\r$ {}", " ".repeat(current_input.len()));
                                if let Some(next) = history.get_next() {
                                    print!("\r$ {}", next);
                                    current_input = next.clone();
                                } else {
                                    print!("\r$ ");
                                    current_input.clear();
                                }
                                stdout.flush().unwrap();
                            }
                            _ => {}
                        }
                        continue 'input;
                    }
                }
                Some(Ok(127)) => { 
                    if !current_input.is_empty() {
                        current_input.pop();
                        print!("\r$ {}", " ".repeat(current_input.len() + 1));
                        print!("\r$ {}", current_input);
                        stdout.flush().unwrap();
                    }
                }
                Some(Ok(c)) => {
                    current_input.push(c as char);
                    print!("{}", c as char);
                    stdout.flush().unwrap();
                }
                _ => {}
            }
        }

        if current_input.trim().is_empty() {
            continue;
        }

        if !current_input.trim().is_empty() {
            history.add(current_input.clone());
        }

        let lexer = Lexer::new(&current_input, env_vars.clone());
        let tokens: Vec<TokenType> = lexer.into_iter().collect();

        if tokens.is_empty() {
            continue;
        }
        let pipelines = parse_pipeline(tokens);
        if pipelines.is_empty() {
            continue;
        }

        let mut last_success = true;
        'pipeline_loop: for (pipeline, operator) in pipelines {
            if pipeline.len() == 1 {
                let cmd = &pipeline[0];
                match cmd.command.as_str() {
                    "exit" => {
                        process::exit(cmd.args.first().and_then(|s| s.parse().ok()).unwrap_or(0))
                    }
                    "cd" => {
                        let path = cmd.args.first().map(String::as_str).unwrap_or("");
                        last_success = if path.is_empty() {
                            let home = env_vars.get("HOME").cloned().unwrap_or_default();
                            change_directory(&home).is_ok()
                        } else {
                            change_directory(path).is_ok()
                        };
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

                match execute_command(
                    &cmd.command,
                    &cmd.args,
                    &env_path,
                    redirection,
                    previous_output,
                ) {
                    Ok((output, success)) => {
                        previous_output = output;
                        last_success = success;
                    }
                    Err(e) => {
                        eprintln!("Error executing command: {}", e);
                        last_success = false;
                        break;
                    }
                }
            }

            match operator {
                Some(TokenType::And) if !last_success => break 'pipeline_loop,
                Some(TokenType::Or) if last_success => break 'pipeline_loop,
                _ => {}
            }
        }
    }
}

// earlier mode of redirection --saved for reference
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

// Basic version of lexer (if you want to implement using this)
// fn tokenize(input: &str) -> Vec<String> {
//     let mut tokens = Vec::new();
//     let mut current_token = String::new();
//     let mut in_single_quotes = false;
//     let mut in_double_quotes = false;
//     let mut chars = input.chars().peekable();
//     let mut escaped = false;

//     while let Some(c) = chars.next() {
//         match c {
//             '\\' if !in_single_quotes => {
//                 if let Some(&next_char) = chars.peek() {
//                     if in_double_quotes {
//                         match next_char {
//                             '\\' | '$' | '"' | '\n' => {
//                                 chars.next();
//                                 current_token.push(next_char);
//                             }
//                             _ => {
//                                 current_token.push('\\');
//                                 current_token.push(next_char);
//                                 chars.next();
//                             }
//                         }
//                     } else {
//                         chars.next();
//                         current_token.push(next_char);
//                     }
//                 } else {
//                     current_token.push('\\');
//                 }
//             }
//             '\'' if !escaped && !in_double_quotes => {
//                 in_single_quotes = !in_single_quotes;
//             }
//             '"' if !escaped && !in_single_quotes => {
//                 in_double_quotes = !in_double_quotes;
//             }
//             ' ' if !escaped && !in_single_quotes && !in_double_quotes => {
//                 if !current_token.is_empty() {
//                     tokens.push(current_token.clone());
//                     current_token.clear();
//                 }
//             }
//             _ => {
//                 current_token.push(c);
//             }
//         }
//         escaped = false;
//     }

//     if !current_token.is_empty() {
//         tokens.push(current_token);
//     }

//     tokens.into_iter().filter(|s| !s.is_empty()).collect()
// }
