use std::error::Error;
use std::env;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

mod caster;
mod receiver;
mod ui;

use ui::MyApp;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <caster|receiver|ui>", args[0]);
        std::process::exit(1);
    }

    match args[1].as_str() {
        "caster" => {
            let addr = "127.0.0.1:12345";
            println!("Avviando il caster...");
            let stop_signal = Arc::new(AtomicBool::new(false));
            caster::start_caster(addr, stop_signal).await?;
        }
        "receiver" => {
            let addr = "127.0.0.1:12345";
            println!("Avviando il receiver...");
            let stop_signal = Arc::new(AtomicBool::new(false));
            receiver::receive_frame(addr, stop_signal).await?;
        }
        "ui" => {
            let options = eframe::NativeOptions::default();
            eframe::run_native("Screencast App", options, Box::new(|_cc| Ok(Box::new(MyApp::default()))))?;
        }
        _ => {
            eprintln!("Usage: {} <caster|receiver|ui>", args[0]);
            std::process::exit(1);
        }
    }
    Ok(())
}