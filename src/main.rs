use std::error::Error;
use std::env;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use eframe::egui::ViewportBuilder;

mod caster;
mod receiver;
mod ui;

use ui::MyApp;

use eframe::epaint::Rect;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <caster|receiver|ui>", args[0]);
        std::process::exit(1);
    }

    match args[1].as_str() {
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
