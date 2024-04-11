# Docker Command Executor

This Rust library provides a straightforward way to manage the execution of commands inside Docker containers using the `docker_api` crate. It supports synchronous command execution with optional timeouts and handles container lifecycle management including creation, execution, logging, and cleanup.

## Features

- **Command Execution**: Execute arbitrary commands inside Docker containers.
- **Timeout Support**: Optionally specify a maximum execution time for commands.
- **Automatic Cleanup**: Automatically stop and remove containers after command execution.
- **Logging**: Capture and return stdout and stderr based on command success or failure.

## Prerequisites

- Rust 2018 Edition or later
- Docker daemon accessible via Unix socket or TCP
- `docker_api` crate

## Usage

### Setup

Add this crate to your `Cargo.toml`:

```toml
[dependencies]
docker_api = "0.7"
tokio = { version = "1", features = ["full"] }
```

### Example

Here's a simple example of how to use `DockerExec` to run a command:

```rust
use crate::DockerExec;
use docker_api::Docker;

#[tokio::main]
async fn main() {
    let docker = Docker::new("unix:///var/run/docker.sock").unwrap();
    let image = "alpine".to_owned();
    let command = vec!["echo".to_owned(), "Hello, World!".to_owned()];
    let timeout_secs = Some(10);

    let exec = DockerExec::new(docker, image, command, timeout_secs);
    let output = exec.execute().await.unwrap();

    println!("Output: {}", output);
}

### Methods

- `new(docker: Docker, image: String, command: Vec<String>, timeout_secs: Option<u64>) -> Self`: Creates a new instance of `DockerExec`.
- `execute() -> Result<String>`: Executes the specified command inside a Docker container and returns the output.

### Note

Docker images must be available locally or pulled from a registry before executing commands.

## Testing

The module includes tests demonstrating basic functionality, error handling, and concurrent executions. Ensure you have Docker running and accessible via the specified URI in the environment variable `DOCKER_URI`.

## Error Handling

Errors are managed through the `Result` type, returning either the command output or an error describing the failure, including timeout and execution errors.

## Contribution

Contributions are welcome! Please feel free to submit pull requests or raise issues on the repository.

This library aims to simplify command execution in Docker environments, providing a robust toolset for developers working with containers programmatically.
