use std::process::Command;

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

fn main() {
    let _args = Args::parse();

    let _ = std::thread::Builder::new()
        .name("Renode Spawner".into())
        .spawn(|| {
            if cfg!(target_os = "windows") {
                let _ = Command::new("cmd")
                    .args(["/C", CMD])
                    .spawn()
                    .expect("Didn't spawn renode successfully")
                    .wait();
            } else {
                let _ = Command::new("sh")
                    .arg("-c")
                    .arg(CMD)
                    .spawn()
                    .expect("Didn't spawn renode successfully")
                    .wait();
            };
        });

    QApp::new()
        .register::<Backend>()
        .load_qml(include_bytes!("qml/main.qml"))
        .run();
}
