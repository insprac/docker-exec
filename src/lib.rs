use docker_api::conn::TtyChunk;
use docker_api::errors::Error;
use docker_api::opts::{ContainerCreateOpts, ContainerRemoveOpts, ContainerStopOpts, LogsOpts};
use docker_api::{Container, Docker, Result};
use futures::{Stream, StreamExt};
use std::time::Duration;

pub use docker_api;

/// Manages execution of commands in Docker containers.
///
/// # Example
///
/// ```
/// use docker_exec::{DockerExec, docker_api::Docker};
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() {
///     let docker_uri = std::env::var("DOCKER_URI").expect("DOCKER_URI must be set");
///     let docker = Docker::new(docker_uri).unwrap();
///     let exec = DockerExec::new(
///         docker,
///         "alpine".to_string(),
///         vec!["echo".to_string(), "Hello".to_string()],
///         Some(Duration::from_secs(10)),
///     );
///     let output = exec.execute().await.unwrap();
///     assert_eq!(output, "Hello");
/// }
/// ```
pub struct DockerExec {
    docker: Docker,
    image: String,
    command: Vec<String>,
    timeout: Option<Duration>,
}

impl DockerExec {
    /// Constructs a new `DockerExec`.
    pub fn new(
        docker: Docker,
        image: String,
        command: Vec<String>,
        timeout: Option<Duration>,
    ) -> Self {
        DockerExec {
            docker,
            image,
            command,
            timeout,
        }
    }

    /// Executes the command in the Docker container.
    ///
    /// Does the following:
    /// - Create a new container with the provided image and command.
    /// - Runs the command (optionally with a timeout).
    /// - Removes the container from Docker.
    pub async fn execute(&self) -> Result<String> {
        let container = self.create_container().await?;
        let result = self.run_with_optional_timeout(&container).await;
        self.cleanup(container).await?;
        result
    }

    /// Creates a Docker container for the command execution.
    async fn create_container(&self) -> Result<Container> {
        let opts = ContainerCreateOpts::builder()
            .image(&self.image)
            .command(self.command.clone())
            .build();
        self.docker.containers().create(&opts).await
    }

    /// Runs the container and manages the optional timeout.
    async fn run_with_optional_timeout(&self, container: &Container) -> Result<String> {
        match self.timeout {
            Some(duration) => tokio::time::timeout(duration, self.start_and_wait(container))
                .await
                .map_err(|_| Error::StringError("Execution timed out".to_string()))?,
            None => self.start_and_wait(container).await,
        }
    }

    /// Starts the container and waits for the command to complete.
    async fn start_and_wait(&self, container: &Container) -> Result<String> {
        container.start().await?;
        let wait_status = container.wait().await?;

        if wait_status.status_code != 0 {
            Err(Error::StringError(format!(
                "Command failed with status code: {}\n{}",
                wait_status.status_code,
                self.fetch_logs(container, true).await?
            )))
        } else {
            self.fetch_logs(container, false).await
        }
    }

    /// Fetches logs from the container.
    async fn fetch_logs(&self, container: &Container, include_stderr: bool) -> Result<String> {
        let opts = LogsOpts::builder()
            .stdout(true)
            .stderr(include_stderr)
            .build();
        let log_stream = container.logs(&opts);
        DockerExec::collect_logs(log_stream).await
    }

    /// Collects logs from the log stream.
    async fn collect_logs(
        mut stream: impl Stream<Item = Result<TtyChunk>> + Unpin,
    ) -> Result<String> {
        let mut output = String::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let text = std::str::from_utf8(&chunk.as_slice())
                .map_err(|_| Error::StringError("Failed to parse chunk".to_string()))?;
            output.push_str(text);
        }
        Ok(output.trim().to_string())
    }

    /// Cleans up the container by stopping and removing it.
    async fn cleanup(&self, container: Container) -> Result<String> {
        let _ = container.stop(&ContainerStopOpts::default()).await;
        container
            .remove(&ContainerRemoveOpts::builder().force(true).build())
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use docker_api::Docker;

    fn docker_instance() -> Docker {
        Docker::new(
            &std::env::var("DOCKER_URI")
                .unwrap_or_else(|_| "unix:///var/run/docker.sock".to_string()),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn success() {
        let docker = docker_instance();
        let exec = DockerExec::new(
            docker,
            "alpine".to_string(),
            vec!["echo".to_string(), "successful test".to_string()],
            Some(Duration::from_secs(10)),
        );
        assert_eq!(exec.execute().await.unwrap(), "successful test");
    }

    #[tokio::test]
    async fn success_without_timeout() {
        let docker = docker_instance();
        let exec = DockerExec::new(
            docker,
            "alpine".to_string(),
            vec!["echo".to_string(), "no timeout".to_string()],
            None,
        );
        assert_eq!(exec.execute().await.unwrap(), "no timeout");
    }

    #[tokio::test]
    async fn error_on_exit_code() {
        let docker = docker_instance();
        let exec = DockerExec::new(
            docker,
            "alpine".to_string(),
            vec!["sh".to_string(), "-c".to_string(), "exit 1".to_string()],
            Some(Duration::from_secs(10)),
        );
        let error = exec.execute().await.unwrap_err();
        assert!(error
            .to_string()
            .contains("Command failed with status code: 1"));
    }

    #[tokio::test]
    async fn invalid_command() {
        let docker = docker_instance();
        let exec = DockerExec::new(
            docker,
            "alpine".to_string(),
            vec!["not_a_real_command".to_string()],
            Some(Duration::from_secs(10)),
        );
        assert!(exec.execute().await.is_err());
    }

    #[tokio::test]
    async fn concurrent_executions() {
        let docker = docker_instance();
        let exec1 = DockerExec::new(
            docker.clone(),
            "alpine".to_string(),
            vec!["echo".to_string(), "test1".to_string()],
            Some(Duration::from_secs(10)),
        );
        let exec2 = DockerExec::new(
            docker,
            "alpine".to_string(),
            vec!["echo".to_string(), "test2".to_string()],
            Some(Duration::from_secs(10)),
        );

        let (result1, result2) = tokio::join!(exec1.execute(), exec2.execute());
        assert_eq!(result1.unwrap(), "test1");
        assert_eq!(result2.unwrap(), "test2");
    }

    #[tokio::test]
    async fn command_timeout() {
        let docker = docker_instance();
        let exec = DockerExec::new(
            docker,
            "alpine".to_string(),
            vec!["sleep".to_string(), "5".to_string()],
            Some(Duration::from_secs(3)),
        );
        let result = exec.execute().await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Execution timed out");
    }
}
