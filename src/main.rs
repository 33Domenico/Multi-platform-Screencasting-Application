use std::error::Error;
use std::env;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

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
        "caster" => {
            let addr = "127.0.0.1:12345";
            println!("Avviando il caster...");
            let stop_signal = Arc::new(AtomicBool::new(false));

            // Definizione di un'area selezionata simulata
            let selected_area = Some(Rect::from_min_max(
                egui::pos2(100.0, 100.0),  // Minimo (x0, y0)
                egui::pos2(400.0, 300.0)   // Massimo (x1, y1)
            ));

            caster::start_caster(addr, stop_signal, selected_area).await?;
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
