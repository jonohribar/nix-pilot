//! Shared subprocess runner for Nix adapters.
//!
//! Captures both output streams concurrently, applies a deadline, and returns
//! reader failures instead of silently turning them into empty diagnostics.
use std::fmt;
use std::io::{self, Read};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Completed command output.
pub struct Output {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

/// Failure while starting or collecting a subprocess.
#[derive(Debug)]
pub enum Error {
    Spawn(io::Error),
    Wait(io::Error),
    Timeout,
    Read {
        stream: &'static str,
        source: io::Error,
    },
    ReaderPanicked {
        stream: &'static str,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Spawn(source) => write!(f, "could not start process: {source}"),
            Error::Wait(source) => write!(f, "could not wait for process: {source}"),
            Error::Timeout => f.write_str("process exceeded its time limit"),
            Error::Read { stream, source } => write!(f, "could not read {stream}: {source}"),
            Error::ReaderPanicked { stream } => write!(f, "{stream} reader thread panicked"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Spawn(source) | Error::Wait(source) | Error::Read { source, .. } => Some(source),
            Error::Timeout | Error::ReaderPanicked { .. } => None,
        }
    }
}

type Reader = JoinHandle<io::Result<String>>;

/// Spawn a reader thread for a pipe.
pub fn spawn_reader(pipe: impl Read + Send + 'static) -> Reader {
    thread::spawn(move || {
        let mut output = String::new();
        let mut pipe = pipe;
        pipe.read_to_string(&mut output)?;
        Ok(output)
    })
}

/// Collect output from a reader thread.
pub fn collect_reader(reader: Option<Reader>, stream: &'static str) -> Result<String, Error> {
    match reader {
        Some(reader) => reader
            .join()
            .map_err(|_| Error::ReaderPanicked { stream })?
            .map_err(|source| Error::Read { stream, source }),
        None => Ok(String::new()),
    }
}

/// Discard a reader thread (join without collecting).
pub fn discard_reader(reader: Option<Reader>) {
    if let Some(reader) = reader {
        let _ = reader.join();
    }
}

/// Terminate a child process.
pub fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Run a command with piped, concurrently drained output and a wall-clock limit.
pub fn run(command: &mut Command, timeout: Duration) -> Result<Output, Error> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(Error::Spawn)?;
    let stdout = child.stdout.take().map(spawn_reader);
    let stderr = child.stderr.take().map(spawn_reader);
    let deadline = Instant::now() + timeout;

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(50));
            }
            Ok(None) => {
                terminate(&mut child);
                discard_reader(stdout);
                discard_reader(stderr);
                return Err(Error::Timeout);
            }
            Err(source) => {
                terminate(&mut child);
                discard_reader(stdout);
                discard_reader(stderr);
                return Err(Error::Wait(source));
            }
        }
    };

    Ok(Output {
        status,
        stdout: collect_reader(stdout, "stdout")?,
        stderr: collect_reader(stderr, "stderr")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_both_streams() {
        let output = run(
            Command::new("sh")
                .arg("-c")
                .arg("printf out; printf err >&2"),
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, "out");
        assert_eq!(output.stderr, "err");
    }

    #[test]
    fn drains_output_larger_than_pipe_capacity() {
        let output = run(
            Command::new("sh").arg("-c").arg(
                "head -c 131072 /dev/zero | tr '\\000' x; head -c 131072 /dev/zero | tr '\\000' y >&2",
            ),
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout.len(), 131072);
        assert_eq!(output.stderr.len(), 131072);
    }

    #[test]
    fn terminates_process_at_deadline() {
        let result = run(
            Command::new("sh").arg("-c").arg("exec sleep 5"),
            Duration::from_millis(50),
        );
        assert!(matches!(result, Err(Error::Timeout)));
    }
}
