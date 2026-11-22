//! Shared subprocess runner for Nix adapters.
//!
//! Streams optional stdin while capturing both output streams concurrently,
//! applies a deadline, and reports read/write failures instead of silently
//! turning them into empty diagnostics.
use std::fmt;
use std::io::{self, Read, Write};
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
    /// Writing the command's input stream failed.
    Write(io::Error),
    /// The deadline elapsed; the child was terminated and output discarded.
    Timeout,
    /// Reading one of the child's output pipes failed.
    Read {
        stream: &'static str,
        source: io::Error,
    },
    /// The thread draining the named output pipe panicked.
    ReaderPanicked { stream: &'static str },
    /// The thread writing the child's input stream panicked.
    WriterPanicked,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Spawn(source) => write!(f, "could not start process: {source}"),
            Error::Wait(source) => write!(f, "could not wait for process: {source}"),
            Error::Write(source) => write!(f, "could not write process stdin: {source}"),
            Error::Timeout => f.write_str("process exceeded its time limit"),
            Error::Read { stream, source } => write!(f, "could not read {stream}: {source}"),
            Error::ReaderPanicked { stream } => write!(f, "{stream} reader thread panicked"),
            Error::WriterPanicked => f.write_str("stdin writer thread panicked"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Spawn(source)
            | Error::Wait(source)
            | Error::Write(source)
            | Error::Read { source, .. } => Some(source),
            Error::Timeout | Error::ReaderPanicked { .. } | Error::WriterPanicked => None,
        }
    }
}

type Reader = JoinHandle<io::Result<String>>;
type Writer = JoinHandle<io::Result<()>>;

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

fn collect_writer(writer: Option<Writer>) -> Result<(), Error> {
    match writer {
        Some(writer) => writer
            .join()
            .map_err(|_| Error::WriterPanicked)?
            .map_err(Error::Write),
        None => Ok(()),
    }
}

fn discard_writer(writer: Option<Writer>) {
    if let Some(writer) = writer {
        let _ = writer.join();
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
    run_inner(command, None, timeout)
}

/// Run a command while streaming `stdin_data` to its standard input.
///
/// The input writer and output readers run concurrently with the child, so a
/// command can consume large input while producing output without deadlock.
/// The deadline starts after the child is spawned and limits input delivery
/// and process completion.
///
/// # Errors
/// Returns [`Error`] if spawning, writing, waiting, draining output, or
/// meeting the deadline fails.
pub fn run_with_stdin(
    command: &mut Command,
    stdin_data: &[u8],
    timeout: Duration,
) -> Result<Output, Error> {
    run_inner(command, Some(stdin_data.to_vec()), timeout)
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

fn run_inner(
    command: &mut Command,
    stdin_data: Option<Vec<u8>>,
    timeout: Duration,
) -> Result<Output, Error> {
    if stdin_data.is_some() {
        command.stdin(Stdio::piped());
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(Error::Spawn)?;
    let stdin_writer = stdin_data.and_then(|data| {
        child
            .stdin
            .take()
            .map(|mut stdin| thread::spawn(move || stdin.write_all(&data)))
    });
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
                discard_writer(stdin_writer);
                discard_reader(stdout);
                discard_reader(stderr);
                return Err(Error::Timeout);
            }
            Err(source) => {
                terminate(&mut child);
                discard_writer(stdin_writer);
                discard_reader(stdout);
                discard_reader(stderr);
                return Err(Error::Wait(source));
            }
        }
    };

    let stdin_result = collect_writer(stdin_writer);
    let stdout_result = collect_reader(stdout, "stdout");
    let stderr_result = collect_reader(stderr, "stderr");
    stdin_result?;
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

    #[test]
    fn streams_stdin_to_the_child() {
        let output = run_with_stdin(
            Command::new("sh").arg("-c").arg("cat"),
            b"candidate archive",
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, "candidate archive");
    }

    #[test]
    fn drains_large_output_while_streaming_large_stdin() {
        let input = vec![b'x'; 1024 * 1024];
        let output = run_with_stdin(
            Command::new("sh").arg("-c").arg("cat"),
            &input,
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout.len(), input.len());
        assert!(output.stdout.bytes().all(|byte| byte == b'x'));
    }

    #[test]
    fn reports_stdin_write_failures() {
        let input = vec![b'x'; 2 * 1024 * 1024];
        let result = run_with_stdin(
            Command::new("sh").arg("-c").arg("exec 0<&-; sleep 0.1"),
            &input,
            Duration::from_secs(2),
        );
        assert!(matches!(result, Err(Error::Write(_))));
    }

    #[test]
    fn stdin_writer_is_stopped_when_the_command_times_out() {
        let input = vec![b'x'; 8 * 1024 * 1024];
        let result = run_with_stdin(
            Command::new("sh").arg("-c").arg("exec sleep 5"),
            &input,
            Duration::from_millis(50),
        );
        assert!(matches!(result, Err(Error::Timeout)));
    }
}
