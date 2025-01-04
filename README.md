# RustShell - A Modern Shell Implementation in Pure Rust

A feature-rich shell implementation written in Rust using only the standard library. This project demonstrates systems programming concepts, command parsing, process management, and Rust's safety features.

## Features

- **Pure Rust Implementation**: Built using only the Rust standard library, demonstrating deep understanding of systems programming
- **Advanced Lexer & Parser**: 
  - Handles complex command syntax including pipes, redirections, and logical operators
  - Supports single and double quotes with proper escaping
  - Environment variable expansion
- **Process Management**:
  - Pipe chains (`cmd1 | cmd2 | cmd3`)
  - Input/Output redirection (`>`, `>>`, `2>`, `2>>`)
  - Logical operators (`&&`, `||`)
  - Command separation (`;`)
- **Built-in Commands**:
  - `cd` with home directory expansion (`~`)
  - `exit` with optional status code
- **Error Handling**: Robust error handling using Rust's Result type
- **Cross-Platform**: Works on Unix-like systems with partial Windows support

## Technical Implementation

### Lexical Analysis

The lexer is implemented using a custom iterator-based approach that processes input character by character:

```rust
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
```

- Handles complex token types including quoted strings and redirections
- Supports environment variable expansion within double quotes
- Implements proper escape sequence handling

### Command Parsing

The parser converts tokens into executable commands using a multi-stage approach:

```rust
struct PipelineCommand {
    command: String,
    args: Vec<String>,
    redirection: Redirection,
}
```

- Builds command pipelines with proper argument handling
- Manages redirections and pipe connections
- Supports logical operators for conditional execution

### Process Execution

Advanced process management features:

- Proper stdin/stdout/stderr handling for pipes
- File descriptor management for redirections
- Path resolution for command execution
- Permission checking on Unix systems

## Architecture Highlights

1. **Iterator-based Lexer**: The lexer implements the Iterator trait for efficient token generation
2. **Enum-based Token System**: Uses Rust's powerful enum system for type-safe token representation
3. **Zero-copy String Handling**: Efficient string management using Rust's ownership system
4. **Error Propagation**: Leverages Rust's Result type for robust error handling
5. **Memory Safety**: No unsafe code, demonstrating Rust's safety guarantees
6. **Resource Management**: RAII-based handling of file descriptors and processes

## Performance Considerations

- Zero-allocation path for simple commands
- Efficient string handling using Rust's String type
- Minimal system calls through careful process management
- Smart handling of pipe buffers to prevent deadlocks

## Code Organization

```
src/
├── main.rs   -- contains all the code for now 
```

## Technical Challenges Solved

1. **Proper Quote Handling**: 
   - Differentiates between single and double quotes
   - Handles nested quotes and escape sequences
   - Supports environment variable expansion in double quotes

2. **Pipeline Implementation**:
   - Correct handling of multiple connected processes
   - Proper cleanup of file descriptors
   - Prevention of zombie processes

3. **Redirection Management**:
   - Support for all standard redirections
   - Atomic file operations for redirections
   - Proper error propagation

## Future Enhancements

- Job control support
- Command history
- Tab completion
- Script execution
- More built-in commands
- Better Windows support

## Building and Running

```bash
# Build the project
cargo build --release

# Run the shell
cargo run --release
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
