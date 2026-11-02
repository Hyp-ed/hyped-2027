#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::{Child, Command};

use clap::Parser;
use qtbridge::{QApp, qobject};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {}

const CMD: &str = "renode -P 1234";

#[derive(Default)]
pub struct Backend {}

#[qobject(Singleton)]
impl Backend {
    #[qslot]
    fn say_hello(&self) {
        println!("Hello World");
    }
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
        #[cfg(unix)]
        let mut cmd = {
            let mut cmd = Command::new("sh");
            // Own process group, so that kill_tree can reach everything Renode starts
            cmd.arg("-c").arg(CMD).process_group(0);
            cmd
        };
        #[cfg(windows)]
        let mut cmd = {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", CMD]);
            cmd
        };

        Self(cmd.spawn().expect("Didn't spawn renode successfully"))
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

    let renode = Renode::spawn();

    // Signals end the process without running destructors
    let pid = renode.0.id();
    ctrlc::set_handler(move || {
        kill_tree(pid);
        std::process::exit(130);
    })
    .expect("Didn't set the signal handler successfully");

    QApp::new()
        .register::<Backend>()
        .load_qml(include_bytes!("qml/main.qml"))
        .run();
}
