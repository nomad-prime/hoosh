// Persistent shell session inside a PTY.
//
// One `$SHELL -i` is spawned at startup and reused for every command in the
// REPL. State that depends on a single shell session — `cd`, exports, aliases,
// shell functions, history — survives across commands, the same way it would
// in a normal terminal.
//
// Each user command is framed with two sentinels written by the shell itself:
//
//     printf '__HOOSH_START__\n'; <user line>; printf '\n__HOOSH_DONE_%s__\n' $?
//
// A reader thread watches the PTY output for these markers. Bytes between
// START and DONE are streamed to the user's terminal and into a ring buffer;
// everything else (e.g. the shell's idle prompt) is dropped. The DONE marker
// carries the exit code.
//
// Input echo on the slave PTY is disabled at startup (`stty -echo`) so the
// shell does not double-echo what we already showed at the hoosh prompt.

use anyhow::{Context, Result};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use super::ring_buffer::RingBuffer;

pub struct PtyOutcome {
    pub exit_code: Option<i32>,
    pub output_tail: String,
}

// Markers are per-session and stamped with a random id. The shell expands
// `${HOOSH_MARK}` at runtime; our written stdin only contains the literal
// placeholder, so even if the PTY line discipline echoes our input back to
// us, the parser does not see a false match — only the shell's actual output
// (which has the id substituted) does.
const SENTINEL_HOLDBACK: usize = 96;

enum Chunk {
    Bytes(Vec<u8>),
    Done(i32),
}

type SharedWriter = Arc<Mutex<Box<dyn Write + Send>>>;

pub struct PtySession {
    writer: SharedWriter,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    output_rx: Receiver<Chunk>,
    mark_id: String,
    _child: Box<dyn Child + Send + Sync>,
    _reader_thread: thread::JoinHandle<()>,
}

struct Markers {
    start: Vec<u8>,
    done_prefix: Vec<u8>,
    done_suffix: Vec<u8>,
    ready: Vec<u8>,
}

impl Markers {
    fn for_id(id: &str) -> Self {
        Self {
            start: format!("__HOOSH_{}_START__", id).into_bytes(),
            done_prefix: format!("__HOOSH_{}_DONE_", id).into_bytes(),
            done_suffix: b"__".to_vec(),
            ready: format!("__HOOSH_{}_READY__", id).into_bytes(),
        }
    }
}

impl PtySession {
    pub fn spawn() -> Result<Self> {
        let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("openpty failed")?;

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        // TERM=dumb at startup makes zsh skip ZLE and bash skip readline, so our
        // injected stdin isn't redrawn / syntax-highlighted into the output. We
        // restore TERM via the first command so child processes (vim, less, ...)
        // see a normal terminal.
        let real_term = std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".into());
        let mark_id = uuid::Uuid::new_v4().simple().to_string();
        let shell_name = std::path::Path::new(&shell)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let edit_flag = match shell_name.as_str() {
            "zsh" => " +Z",
            "bash" => " --noediting",
            _ => "",
        };
        let inner = format!(
            "stty -echo 2>/dev/null; TERM=dumb exec {}{} -i",
            shell, edit_flag
        );
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.arg("-c");
        cmd.arg(&inner);
        if let Ok(cwd) = std::env::current_dir() {
            cmd.cwd(cwd);
        }
        for (k, v) in std::env::vars() {
            if k != "TERM" {
                cmd.env(k, v);
            }
        }
        cmd.env("TERM", "dumb");
        cmd.env("HOOSH_REAL_TERM", &real_term);
        cmd.env("HOOSH_MARK", &mark_id);

        let child = pair
            .slave
            .spawn_command(cmd)
            .context("spawn shell failed")?;
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .context("clone reader failed")?;
        let writer_box = pair.master.take_writer().context("take writer failed")?;
        let writer: SharedWriter = Arc::new(Mutex::new(writer_box));

        let master = Arc::new(Mutex::new(pair.master));

        // Disable input echo and announce readiness. We swallow all PTY output
        // until the READY marker arrives — that drains the shell's startup
        // banner, the rc-file output, and the first prompt.
        {
            let mut w = writer.lock().unwrap();
            w.write_all(
                b"stty -echo 2>/dev/null; export TERM=\"${HOOSH_REAL_TERM:-xterm-256color}\"; printf '__HOOSH_%s_READY__\\n' \"$HOOSH_MARK\"\n",
            )
            .context("write init failed")?;
            w.flush().ok();
        }

        let markers = Markers::for_id(&mark_id);
        let (tx, rx) = channel::<Chunk>();
        let reader_thread = spawn_reader_thread(reader, tx, markers);

        // Drain until READY: the reader is in Idle state and silently drops
        // everything, but it also drops the READY marker itself once seen.
        // We don't actually need to coordinate — the next run_command's
        // START marker is what gates output to the user.
        Ok(Self {
            writer,
            master,
            output_rx: rx,
            mark_id,
            _child: child,
            _reader_thread: reader_thread,
        })
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        if let Ok(master) = self.master.lock() {
            let _ = master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
    }

    pub fn run_command(&mut self, line: &str, tail_capacity: usize) -> Result<PtyOutcome> {
        use std::io::IsTerminal;
        let stdin_is_tty = std::io::stdin().is_terminal();
        let raw_enabled = if stdin_is_tty {
            crossterm::terminal::enable_raw_mode().is_ok()
        } else {
            false
        };

        let stop = Arc::new(AtomicBool::new(false));
        let stdin_thread = if stdin_is_tty {
            Some(spawn_stdin_thread(
                Arc::clone(&self.writer),
                Arc::clone(&stop),
            ))
        } else {
            None
        };

        let _ = &self.mark_id;
        // One-line framing keeps the shell's idle prompt from being printed
        // between START and DONE. Trailing $? captures the exit code of the
        // user's command (the last component before the final printf).
        let framed = format!(
            "printf '__HOOSH_'\"$HOOSH_MARK\"'_START__\\n'; {}; printf '\\n__HOOSH_'\"$HOOSH_MARK\"'_DONE_%s__\\n' $?\n",
            line
        );
        {
            let mut w = self.writer.lock().unwrap();
            w.write_all(framed.as_bytes())
                .context("write command failed")?;
            w.flush().ok();
        }

        let mut tail = RingBuffer::new(tail_capacity);
        let stdout = std::io::stdout();
        let mut exit_code: Option<i32> = None;

        while let Ok(chunk) = self.output_rx.recv_timeout(Duration::from_secs(3600)) {
            match chunk {
                Chunk::Bytes(b) => {
                    {
                        let mut lock = stdout.lock();
                        let _ = lock.write_all(&b);
                        let _ = lock.flush();
                    }
                    tail.push(&b);
                }
                Chunk::Done(code) => {
                    exit_code = Some(code);
                    break;
                }
            }
        }

        stop.store(true, Ordering::Relaxed);
        if let Some(h) = stdin_thread {
            let _ = h.join();
        }
        if raw_enabled {
            crossterm::terminal::disable_raw_mode().ok();
        }

        Ok(PtyOutcome {
            exit_code,
            output_tail: tail.as_string(),
        })
    }
}

fn spawn_reader_thread(
    mut reader: Box<dyn Read + Send>,
    tx: Sender<Chunk>,
    markers: Markers,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        let mut pending: Vec<u8> = Vec::new();
        let mut running = false;

        loop {
            let n = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            pending.extend_from_slice(&buf[..n]);

            loop {
                if !running {
                    if let Some(idx) = find_subslice(&pending, &markers.start) {
                        pending.drain(..idx + markers.start.len());
                        running = true;
                        continue;
                    }
                    if let Some(idx) = find_subslice(&pending, &markers.ready) {
                        pending.drain(..idx + markers.ready.len());
                        continue;
                    }
                    if pending.len() > SENTINEL_HOLDBACK {
                        let split = pending.len() - SENTINEL_HOLDBACK;
                        pending.drain(..split);
                    }
                    break;
                }

                if let Some(start) = find_subslice(&pending, &markers.done_prefix) {
                    let after_prefix = start + markers.done_prefix.len();
                    if let Some(rel) = find_subslice(&pending[after_prefix..], &markers.done_suffix)
                    {
                        let code_bytes = &pending[after_prefix..after_prefix + rel];
                        let code = std::str::from_utf8(code_bytes)
                            .ok()
                            .and_then(|s| s.parse::<i32>().ok())
                            .unwrap_or(-1);
                        if start > 0 && tx.send(Chunk::Bytes(pending[..start].to_vec())).is_err() {
                            return;
                        }
                        let end = after_prefix + rel + markers.done_suffix.len();
                        pending.drain(..end);
                        if tx.send(Chunk::Done(code)).is_err() {
                            return;
                        }
                        running = false;
                        continue;
                    }
                    if start > 0 {
                        if tx.send(Chunk::Bytes(pending[..start].to_vec())).is_err() {
                            return;
                        }
                        pending.drain(..start);
                    }
                    break;
                }

                if pending.len() > SENTINEL_HOLDBACK {
                    let split = pending.len() - SENTINEL_HOLDBACK;
                    let bytes = pending[..split].to_vec();
                    pending.drain(..split);
                    if tx.send(Chunk::Bytes(bytes)).is_err() {
                        return;
                    }
                }
                break;
            }
        }
    })
}

#[cfg(unix)]
fn spawn_stdin_thread(writer: SharedWriter, stop: Arc<AtomicBool>) -> thread::JoinHandle<()> {
    use nix::poll::{PollFd, PollFlags, poll};
    use std::os::fd::BorrowedFd;
    use std::os::unix::io::AsRawFd;

    thread::spawn(move || {
        let stdin = std::io::stdin();
        let fd_raw = stdin.as_raw_fd();
        let mut buf = [0u8; 1024];

        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let borrowed = unsafe { BorrowedFd::borrow_raw(fd_raw) };
            let mut fds = [PollFd::new(borrowed, PollFlags::POLLIN)];
            match poll(&mut fds, 5u16) {
                Ok(0) => continue,
                Ok(_) => {
                    let revents = fds[0].revents().unwrap_or(PollFlags::empty());
                    if !revents.contains(PollFlags::POLLIN) {
                        if revents.intersects(PollFlags::POLLHUP | PollFlags::POLLERR) {
                            break;
                        }
                        continue;
                    }
                    let mut handle = stdin.lock();
                    match handle.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let Ok(mut w) = writer.lock() else { break };
                            if w.write_all(&buf[..n]).is_err() {
                                break;
                            }
                            let _ = w.flush();
                        }
                        Err(_) => break,
                    }
                }
                Err(_) => break,
            }
        }
    })
}

#[cfg(not(unix))]
fn spawn_stdin_thread(writer: SharedWriter, stop: Arc<AtomicBool>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut buf = [0u8; 1024];
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let mut handle = stdin.lock();
            match handle.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let Ok(mut w) = writer.lock() else { break };
                    if w.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    let _ = w.flush();
                }
                Err(_) => break,
            }
        }
    })
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_subslice_basic() {
        assert_eq!(find_subslice(b"hello world", b"world"), Some(6));
        assert_eq!(find_subslice(b"hello world", b"xyz"), None);
        assert_eq!(find_subslice(b"", b"a"), None);
    }

    fn run_one(session: &mut PtySession, cmd: &str) -> PtyOutcome {
        session.run_command(cmd, 4096).unwrap()
    }

    #[test]
    fn echo_runs_and_returns_zero() {
        let mut s = PtySession::spawn().unwrap();
        let o = run_one(&mut s, "echo hello-from-pty-test");
        assert_eq!(o.exit_code, Some(0));
        assert!(
            o.output_tail.contains("hello-from-pty-test"),
            "got: {:?}",
            o.output_tail
        );
    }

    #[test]
    fn unknown_command_returns_127() {
        let mut s = PtySession::spawn().unwrap();
        let o = run_one(&mut s, "definitely_not_a_real_command_xyzzy");
        assert_eq!(o.exit_code, Some(127));
    }

    #[test]
    fn missing_file_returns_non_zero() {
        let mut s = PtySession::spawn().unwrap();
        let o = run_one(&mut s, "cat /definitely/no/such/path/xyzzy");
        assert!(matches!(o.exit_code, Some(c) if c != 0 && c != 127));
    }

    #[test]
    fn cd_persists_across_commands() {
        let mut s = PtySession::spawn().unwrap();
        let _ = run_one(&mut s, "cd /tmp");
        let o = run_one(&mut s, "pwd");
        assert_eq!(o.exit_code, Some(0));
        assert!(
            o.output_tail.contains("/tmp"),
            "expected pwd to show /tmp, got: {:?}",
            o.output_tail
        );
    }

    #[test]
    fn export_persists_across_commands() {
        let mut s = PtySession::spawn().unwrap();
        let _ = run_one(&mut s, "export HOOSH_TEST_VAR=banana");
        let o = run_one(&mut s, "echo \"$HOOSH_TEST_VAR\"");
        assert_eq!(o.exit_code, Some(0));
        assert!(
            o.output_tail.contains("banana"),
            "expected banana, got: {:?}",
            o.output_tail
        );
    }
}
