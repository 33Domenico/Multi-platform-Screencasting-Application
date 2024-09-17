use eframe::egui;
use std::error::Error;
use tokio::runtime::Runtime;
use crate::{caster, receiver};


fn main() -> Result<(), Box<dyn Error>> {
    // Avvia l'applicazione eGUI
    let options = eframe::NativeOptions::default();
    eframe::run_native("Screencast App", options, Box::new(|_cc| Box::new(MyApp::default())))?;
    Ok(())
}

// Enum per rappresentare la modalità
#[derive(Debug, Clone)]
enum Modality {
    Caster,
    Receiver,
}

struct MyApp {
    mode: Option<Modality>, // Uso di Option<Modality> per la modalità
    caster_address: String,
    status_message: String,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            mode: None,
            caster_address: String::from("127.0.0.1:12345"),
            status_message: String::from("Seleziona una modalità per iniziare."),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Screencast Application");

            // Seleziona se eseguire come caster o receiver
            ui.horizontal(|ui| {
                if ui.button("Caster").clicked() {
                    self.mode = Some(Modality::Caster);
                }
                if ui.button("Receiver").clicked() {
                    self.mode = Some(Modality::Receiver);
                }
            });

            // Mostra la modalità selezionata e gestisci l'interazione
            if let Some(ref mode) = self.mode {
                match mode {
                    Modality::Caster => {
                        ui.label("Modalità selezionata: Caster");
                    }
                    Modality::Receiver => {
                        ui.label("Modalità selezionata: Receiver");

                        // Mostra il campo di input per l'indirizzo del caster solo in modalità receiver
                        ui.horizontal(|ui| {
                            ui.label("Indirizzo caster:");
                            ui.text_edit_singleline(&mut self.caster_address);
                        });
                    }
                }

                // Pulsante per avviare il processo
                if ui.button("Avvia").clicked() {
                    self.status_message = match mode {
                        Modality::Caster => {
                            let rt = Runtime::new().unwrap();
                            rt.block_on(async {
                                if let Err(e) = caster::start_caster("127.0.0.1:12345").await {
                                    eprintln!("Errore: {}", e);
                                }
                            });
                            "Caster avviato!".to_string()
                        }
                        Modality::Receiver => {
                            let addr = self.caster_address.clone();
                            let rt = Runtime::new().unwrap();
                            rt.block_on(async {
                                if let Err(e) = receiver::receive_frame(&addr).await {
                                    eprintln!("Errore: {}", e);
                                }
                            });
                            format!("Receiver connesso a {}", self.caster_address)
                        }
                    };
                }
            }

            // Mostra il messaggio di stato
            ui.label(&self.status_message);
        });
    }
}
