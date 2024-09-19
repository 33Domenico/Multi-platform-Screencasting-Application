use eframe::egui;
use crate::{caster, receiver};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

#[derive(Debug, Clone)]
enum Modality {
    Caster,
    Receiver,
}

pub struct MyApp {
    mode: Option<Modality>,
    caster_address: String,
    status_message: String,
    caster_running: bool,
    receiver_running: bool,
    stop_signal: Arc<AtomicBool>,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            mode: None,
            caster_address: String::from("127.0.0.1:12345"),
            status_message: String::from("Seleziona una modalità per iniziare."),
            caster_running: false,
            receiver_running: false,
            stop_signal: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Screencast Application");

            ui.horizontal(|ui| {
                if ui.button("Caster").clicked() {
                    self.mode = Some(Modality::Caster);
                    self.caster_running = false; // Reset caster state when switching modes
                    self.receiver_running = false;
                    self.stop_signal.store(false, Ordering::SeqCst);
                    self.status_message = "Modalità selezionata: Caster".to_string(); // Aggiorna il messaggio di stato
                }
                if ui.button("Receiver").clicked() {
                    self.mode = Some(Modality::Receiver);
                    self.caster_running = false; // Reset caster state when switching modes
                    self.receiver_running = false;
                    self.stop_signal.store(false, Ordering::SeqCst);
                    self.status_message = "Modalità selezionata: Receiver".to_string(); // Aggiorna il messaggio di stato
                }
            });

            if let Some(ref mode) = self.mode {
                match mode {
                    Modality::Caster => {


                        if !self.caster_running {
                            if ui.button("Avvia").clicked() {
                                self.status_message = "Avviando il caster...".to_string();
                                self.caster_running = true;
                                self.stop_signal.store(false, Ordering::SeqCst);

                                let stop_signal = self.stop_signal.clone();
                                let ctx = ctx.clone();
                                std::thread::spawn(move || {
                                    tokio::runtime::Runtime::new().unwrap().block_on(async {
                                        if let Err(e) = caster::start_caster("127.0.0.1:12345", stop_signal).await {
                                            eprintln!("Errore: {}", e);
                                        }
                                    });
                                    ctx.request_repaint();
                                });
                            }
                        } else {
                            if ui.button("Stop").clicked() {
                                self.status_message = "Interrompendo il caster...".to_string();
                                self.stop_signal.store(true, Ordering::SeqCst);
                                self.caster_running = false;
                                self.status_message = "Caster interrotto.".to_string();
                            }
                        }
                    }
                    Modality::Receiver => {


                        ui.horizontal(|ui| {
                            ui.label("Indirizzo caster:");
                            ui.text_edit_singleline(&mut self.caster_address);
                        });

                        if !self.receiver_running {
                            if ui.button("Avvia").clicked() {
                                let addr = self.caster_address.clone();
                                self.status_message = "Connettendo al caster...".to_string();
                                self.receiver_running = true;
                                self.stop_signal.store(false, Ordering::SeqCst);

                                let stop_signal = self.stop_signal.clone();
                                let ctx = ctx.clone();
                                std::thread::spawn(move || {
                                    tokio::runtime::Runtime::new().unwrap().block_on(async {
                                        if let Err(e) = receiver::receive_frame(&addr, stop_signal).await {
                                            eprintln!("Errore: {}", e);
                                        }
                                    });
                                    ctx.request_repaint();
                                });
                            }
                        } else {
                            if ui.button("Stop").clicked() {
                                self.status_message = "Interrompendo il receiver...".to_string();
                                self.stop_signal.store(true, Ordering::SeqCst);
                                self.receiver_running = false;
                            }
                        }
                    }
                }
            }

            ui.label(&self.status_message);
        });
    }
}