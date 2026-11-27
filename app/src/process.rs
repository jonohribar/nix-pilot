//! Shared subprocess runner for Nix adapters.
//!
//! Captures both output streams concurrently, applies a deadline, and reports
//! read failures instead of silently turning them into empty diagnostics.
use std::fmt;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Exit status and fully collected output streams from a completed command.
pub struct Output {
    /// The child's exit status; a non-success status is still a completed run.
    pub status: ExitStatus,
    /// UTF-8 stdout captured while the child was running.
    pub stdout: String,
    /// UTF-8 stderr captured while the child was running.
    pub stderr: String,
}

/// Failure while starting or collecting a subprocess.
#[derive(Debug)]
pub enum Error {
    /// The operating system could not start the command.
    Spawn(io::Error),
    /// Waiting for the child process failed.
    Wait(io::Error),
    /// The deadline elapsed; the child was terminated and output discarded.
    Timeout,
    /// Reading one of the child's output pipes failed.
    Read {
        stream: &'static str,
        source: io::Error,
    },
    /// The thread draining the named output pipe panicked.
    ReaderPanicked { stream: &'static str },
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

/// Drain a pipe into a UTF-8 string on a dedicated thread.
///
/// Concurrent draining prevents a child from blocking when either output pipe
/// fills while the caller waits for process completion.
pub fn spawn_reader(pipe: impl Read + Send + 'static) -> Reader {
    thread::spawn(move || {
        let mut output = String::new();
        let mut pipe = pipe;
        pipe.read_to_string(&mut output)?;
        Ok(output)
    })
}

/// Join a pipe reader and return its output, identifying the stream on failure.
/// A missing reader is treated as an empty stream.
pub fn collect_reader(reader: Option<Reader>, stream: &'static str) -> Result<String, Error> {
    match reader {
        Some(reader) => reader
            .join()
            .map_err(|_| Error::ReaderPanicked { stream })?
            .map_err(|source| Error::Read { stream, source }),
        None => Ok(String::new()),
    }
}

/// Join a reader thread without retaining its output or read error.
pub fn discard_reader(reader: Option<Reader>) {
    if let Some(reader) = reader {
        let _ = reader.join();
    }
}

/// Best-effort kill and reap a child process.
pub fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Run a command with piped, concurrently drained output and a wall-clock limit.
///
/// A non-zero exit code is returned in [`Output::status`], not treated as a
/// runner error. On timeout, the child is killed and both reader threads are
/// joined before returning.
///
/// # Errors
/// Returns [`Error`] if spawning, waiting, draining output, or meeting the
/// deadline fails.
pub fn run(command: &mut Command, timeout: Duration) -> Result<Output, Error> {
    run_inner(command, timeout)
}

/// Open a folder in the desktop's file manager (best effort, via `xdg-open`).
///
/// # Errors
/// Returns an [`Error`] when the command cannot be started or times out.
pub fn open_folder(path: &Path) -> Result<(), Error> {
    run(Command::new("xdg-open").arg(path), Duration::from_secs(10)).map(|_| ())
}

/// Keep only the trailing `lines` of `output`, oldest first, as a single
/// string. Nix diagnostics are most useful at the end; this bounds what the
/// UI surfaces from a long build or activation log.
pub fn tail(output: &str, lines: usize) -> String {
    output
        .lines()
        .rev()
        .take(lines)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n")
}

fn run_inner(command: &mut Command, timeout: Duration) -> Result<Output, Error> {
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

    let stdout_result = collect_reader(stdout, "stdout");
    let stderr_result = collect_reader(stderr, "stderr");
    Ok(Output {
        status,
        stdout: stdout_result?,
        stderr: stderr_result?,
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
