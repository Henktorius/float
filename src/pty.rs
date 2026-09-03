use std::io::{self, Write};
use std::sync::mpsc;

use crate::escape::{fix_escape_sequences, respond_to_queries};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};

pub struct Pty {
    master: Box<dyn MasterPty>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    parser: vt100::Parser,
    rx: mpsc::Receiver<Vec<u8>>,
}

impl Pty {
    pub fn spawn(shell: &str, rows: u16, cols: u16) -> anyhow::Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(shell);
        // Float emulates an xterm-like terminal (via vt100), not the bare Linux
        // console it may run on. If a child would otherwise inherit
        // `TERM=linux`, upgrade it so mouse-aware programs use xterm mouse
        // reporting (which Float forwards) instead of trying to reach gpm on a
        // pty where it cannot work.
        if std::env::var("TERM").is_ok_and(|t| t == "linux" || t.starts_with("linux-")) {
            cmd.env("TERM", "xterm-256color");
        }
        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);

        let writer = pair.master.take_writer()?;
        let mut reader = pair.master.try_clone_reader()?;
        let parser = vt100::Parser::new(rows, cols, 0);

        let (tx, rx) = mpsc::channel::<Vec<u8>>();

        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Pty {
            master: pair.master,
            writer,
            child,
            parser,
            rx,
        })
    }

    pub fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writer.write_all(bytes)
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.parser.screen_mut().set_size(rows, cols);
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    }

    pub fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }

    /// Drain all buffered PTY output into the vt100 parser.
    /// Returns true if any data was consumed.
    pub fn process(&mut self) -> bool {
        let mut got = false;
        while let Ok(bytes) = self.rx.try_recv() {
            let fixed = fix_escape_sequences(&bytes);

            // Feed to parser first so screen state is current when we
            // respond to any terminal queries embedded in the output.
            self.parser.process(&fixed);
            respond_to_queries(&mut self.writer, &fixed, self.parser.screen());
            got = true;
        }
        got
    }

    /// Returns the child process command name from /proc/<pid>/comm, if available.
    pub fn process_name(&self) -> Option<String> {
        let pid = self.child.process_id()?;
        std::fs::read_to_string(format!("/proc/{pid}/comm"))
            .ok()
            .map(|s| s.trim().to_string())
    }

    /// Returns the command name of the foreground process in this PTY,
    /// using `/proc/<pid>/stat` tpgid field to find the foreground process group.
    pub fn foreground_process_name(&self) -> Option<String> {
        let shell_pid = self.child.process_id()?;
        let stat = std::fs::read_to_string(format!("/proc/{shell_pid}/stat")).ok()?;

        let after_comm = stat.find(')')?;
        let rest = &stat[after_comm + 2..]; // skip ") "
        let tpgid: i32 = rest.split(' ').nth(5)?.parse().ok()?;

        if tpgid <= 0 {
            return None;
        }

        std::fs::read_to_string(format!("/proc/{tpgid}/comm"))
            .ok()
            .map(|s| s.trim().to_string())
    }

    pub fn try_wait(&mut self) -> io::Result<Option<portable_pty::ExitStatus>> {
        self.child.try_wait()
    }

    pub fn _kill(&mut self) -> io::Result<()> {
        self.child.kill()
    }
}
