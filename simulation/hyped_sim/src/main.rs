#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::{
    io,
    net::TcpStream,
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};

use clap::Parser;
use qtbridge::QApp;

use crate::backend::Backend;

mod backend;
mod monitor;
mod terminal;
mod uart;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {}

const MONITOR_PORT: u16 = 1234;
const MONITOR_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

fn connect_monitor() -> io::Result<TcpStream> {
    TcpStream::connect(("127.0.0.1", MONITOR_PORT))
}

/// Renode's launcher script doesn't `exec` into dotnet, so the actual simulator is a grandchild
/// and killing only the direct child would leave it running. The whole process tree has to go.
fn kill_tree(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::killpg(pid as libc::pid_t, libc::SIGTERM);
    }
    #[cfg(windows)]
    let _ = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .output();
}

/// Terminates Renode when dropped, so it can't outlive the main thread.
struct Renode(Child);

impl Renode {
    fn spawn() -> Self {
        let cmd_line = format!("renode -P {MONITOR_PORT}");

        #[cfg(unix)]
        let mut cmd = {
            let mut cmd = Command::new("sh");
            // Own process group, so that kill_tree can reach everything Renode starts
            cmd.arg("-c").arg(cmd_line).process_group(0);
            cmd
        };
        #[cfg(windows)]
        let mut cmd = {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", &cmd_line]);
            cmd
        };

        Self(cmd.spawn().expect("Didn't spawn renode successfully"))
    }

    fn wait_for_monitor(&mut self) -> TcpStream {
        let deadline = Instant::now() + MONITOR_STARTUP_TIMEOUT;
        loop {
            match connect_monitor() {
                Ok(monitor) => return monitor,
                Err(e) if Instant::now() >= deadline => {
                    panic!("Didn't connect to the renode monitor in time: {e}")
                }
                Err(_) => {}
            }
            if let Ok(Some(status)) = self.0.try_wait() {
                panic!("Renode exited before opening its monitor: {status}");
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Drop for Renode {
    fn drop(&mut self) {
        kill_tree(self.0.id());
        let _ = self.0.wait();
    }
}

fn main() {
    let _args = Args::parse();

    let (monitor, renode) = match connect_monitor() {
        Ok(monitor) => (monitor, None),
        Err(_) => {
            let mut renode = Renode::spawn();

            // Signals end the process without running destructors
            let pid = renode.0.id();
            ctrlc::set_handler(move || {
                kill_tree(pid);
                std::process::exit(130);
            })
            .expect("Didn't set the signal handler successfully");

            (renode.wait_for_monitor(), Some(renode))
        }
    };

    let mut app = QApp::new();
    let backend = Backend::new(monitor);
    app.set_initial_object("backend", backend)
        .load_qml(include_bytes!("qml/main.qml"))
        .run();

    // Closing Renode wakes the UART threads, which need the QApp to still exist to post to
    drop(renode);
}
