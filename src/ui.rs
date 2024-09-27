use eframe::{egui, App, Frame, CreationContext};
use crate::{caster, receiver};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use eframe::egui::{Rect, Pos2, Color32, UiBuilder};
use tokio::runtime::Runtime;

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
    start_pos: Option<Pos2>,
    selecting_area: bool,
    selected_area: Option<Rect>,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            mode: None,
            caster_address: String::from(""),
            status_message: String::from("Seleziona una modalità per iniziare."),
            caster_running: false,
            receiver_running: false,
            stop_signal: Arc::new(AtomicBool::new(false)),
            start_pos: None,
            selecting_area: false,
            selected_area: None,
        }
    }
}

impl MyApp {
    pub fn new(_cc: &CreationContext<'_>) -> Self {
        Default::default()
    }

    fn handle_selection(&mut self, ctx: &egui::Context) {
        if self.selecting_area {
            let response = ctx.input(|i| {
                let pos = i.pointer.hover_pos();
                let pressed = i.pointer.primary_pressed();
                let released = i.pointer.primary_released();
                (pos, pressed, released)
            });

            if let (Some(current_pos), pressed, released) = response {
                if pressed && self.start_pos.is_none() {
                    self.start_pos = Some(current_pos);
                } else if released && self.start_pos.is_some() {
                    let start = self.start_pos.unwrap();
                    self.selected_area = Some(Rect::from_two_pos(start, current_pos));
                    self.selecting_area = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                    self.status_message = format!("Area selezionata: {:?}", self.selected_area.unwrap());
                }
                if self.selecting_area && (pressed || released) {
                    ctx.request_repaint();
                }
            }

        }
    }
}

impl App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut Frame) {
        if self.selecting_area {
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(Color32::from_rgba_unmultiplied(0, 0, 0, 100)))
                .show(ctx, |ui| {
                    let screen_rect = ui.max_rect();

                    // Calcola la posizione centrale
                    let center_x = screen_rect.center().x;
                    let center_y = screen_rect.center().y;
                    let rect=Rect::from_center_size(
                        Pos2::new(center_x, center_y), // Posizione centrale
                        egui::vec2(200.0, 50.0), // Dimensioni del rettangolo del testo
                    );
                    // Crea un layout personalizzato e centra il testo
                    ui.allocate_new_ui(UiBuilder::max_rect(Default::default(), rect), |ui| {
                            // Centra il testo
                            ui.with_layout(egui::Layout::centered_and_justified(egui::Direction::TopDown), |ui| {
                                ui.colored_label(egui::Color32::WHITE, "Clicca e trascina per selezionare l'area.");
                            });
                        },
                    );


                    self.handle_selection(ctx);
                    if let Some(start) = self.start_pos {
                        if let Some(current) = ui.input(|i| i.pointer.hover_pos()) {
                            let rect = Rect::from_two_pos(start, current);
                            ui.painter().rect_stroke(rect, 0.0, (2.0, egui::Color32::WHITE));
                            ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 50));
                        }
                    }

                });
        } else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Screencast Application");
                ui.horizontal(|ui| {
                    if ui.button("Caster").clicked() {
                        self.mode = Some(Modality::Caster);
                        self.caster_running = false;
                        self.receiver_running = false;
                        self.stop_signal.store(false, Ordering::SeqCst);
                        self.selecting_area = false;
                        self.selected_area = None;
                        self.status_message = "Modalità selezionata: Caster".to_string();
                    }
                    if ui.button("Receiver").clicked() {
                        self.mode = Some(Modality::Receiver);
                        self.caster_running = false;
                        self.receiver_running = false;
                        self.stop_signal.store(false, Ordering::SeqCst);
                        self.selecting_area = false;
                        self.selected_area = None;
                        self.status_message = "Modalità selezionata: Receiver".to_string();
                    }
                });

                if let Some(ref mode) = self.mode {
                    match mode {
                        Modality::Caster => {
                            ui.horizontal(|ui| {
                                ui.label("Indirizzo caster: es.127.0.0.1:12345 in locale o tra più dispositivi 192.168.165.219:8080");
                                ui.text_edit_singleline(&mut self.caster_address);
                            });

                            if !self.caster_running {
                                if ui.button("Avvia").clicked() {
                                    self.status_message = "Avviando il caster...".to_string();
                                    self.caster_running = true;
                                    self.stop_signal.store(false, Ordering::SeqCst);

                                    let stop_signal = self.stop_signal.clone();
                                    let ctx = ctx.clone();
                                    let selected_area = self.selected_area;  // Pass the selected area
                                    let caster_address = self.caster_address.clone();  // Use the IP input

                                    std::thread::spawn(move || {
                                        Runtime::new().unwrap().block_on(async {
                                            if let Err(e) = caster::start_caster(&caster_address, stop_signal, selected_area).await {
                                                eprintln!("Errore: {}", e);
                                            }
                                        });
                                        ctx.request_repaint();
                                    });
                                }

                                if ui.button("Seleziona area").clicked() {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
                                    self.selecting_area = true;
                                    self.start_pos = None;
                                    self.status_message = "Clicca e trascina per selezionare l'area".to_string();
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
                                ui.label("Indirizzo caster: es.127.0.0.1:12345 in locale o tra più dispositivi 192.168.165.219:8080");
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
                                        Runtime::new().unwrap().block_on(async {
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
}