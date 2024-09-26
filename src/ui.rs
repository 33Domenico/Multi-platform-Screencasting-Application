use eframe::{egui, Frame};
use eframe::WindowBuilder;
use crate::{caster, receiver};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use egui::{Context, Widget};
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
    start_pos: Option<egui::Pos2>, // Posizione iniziale per la selezione
    selecting_area: bool,
    selected_area: Option<(f32, f32, f32, f32)>, // Area selezionata
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
            start_pos: None,  // Inizializza la posizione di partenza
            selecting_area: false, // Inizializza a false
            selected_area: None, // Inizializza a None
        }
    }
}

impl MyApp {
    fn handle_selection(&mut self, ctx: &egui::Context) {
        ctx.input(|input| {
            // Primo click - inizia la selezione
            if input.pointer.any_pressed() {
                if let Some(pos) = input.pointer.hover_pos() {
                    self.start_pos = Some(pos);
                }
            }

            // Rilascio - termina la selezione
            if input.pointer.any_released() {
                if let Some(pos) = input.pointer.hover_pos() {
                    if let Some(start) = self.start_pos {
                        let min_x = start.x.min(pos.x);
                        let min_y = start.y.min(pos.y);
                        let width = (start.x - pos.x).abs();
                        let height = (start.y - pos.y).abs();

                        self.selected_area = Some((min_x, min_y, width, height));
                        self.start_pos = None; // Resetta la posizione di inizio
                        self.selecting_area = false; // Disattiva la selezione dopo aver selezionato
                    }
                }
            }

            // Se l'utente sta trascinando, disegna il rettangolo
            if let Some(start) = self.start_pos {
                if let Some(current_pos) = input.pointer.hover_pos() {
                    let rect = egui::Rect::from_two_pos(start, current_pos);
                    ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("selection")))
                        .rect_stroke(rect, 0.0, (2.0, egui::Color32::RED)); // rettangolo rosso
                }
            }
        });
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Screencast Application");

            ui.horizontal(|ui| {
                if ui.button("Caster").clicked() {
                    self.mode = Some(Modality::Caster);
                    self.caster_running = false;
                    self.receiver_running = false;
                    self.stop_signal.store(false, Ordering::SeqCst);
                    self.selecting_area = false; // Assicurati che la selezione sia disattivata
                    self.selected_area = None; // Resetta l'area selezionata
                    self.status_message = "Modalità selezionata: Caster".to_string();
                }
                if ui.button("Receiver").clicked() {
                    self.mode = Some(Modality::Receiver);
                    self.caster_running = false;
                    self.receiver_running = false;
                    self.stop_signal.store(false, Ordering::SeqCst);
                    self.selecting_area = false; // Assicurati che la selezione sia disattivata
                    self.selected_area = None; // Resetta l'area selezionata
                    self.status_message = "Modalità selezionata: Receiver".to_string();
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
                                    Runtime::new().unwrap().block_on(async {
                                        if let Err(e) = caster::start_caster("127.0.0.1:12345", stop_signal).await {
                                            eprintln!("Errore: {}", e);
                                        }
                                    });
                                    ctx.request_repaint();
                                });
                            }

                            // Selezione area di schermo
                            if ui.button("Seleziona area").clicked() {
                                self.selecting_area = true; // Attiva la selezione
                                self.start_pos = None; // Resetta la posizione di inizio
                                self.status_message = "Clicca e trascina per selezionare l'area".to_string();
                            }

                            // Gestisci la selezione dell'area
                            if self.selecting_area {
                                self.handle_selection(ctx); // Chiama la funzione per gestire la selezione

                                // Se l'area è stata selezionata, visualizza le coordinate
                                if let Some((x, y, width, height)) = self.selected_area {
                                    ui.label(format!("Area selezionata: ({}, {}, {}, {})", x, y, width, height));
                                }
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


/*
pub struct WindowPortion {
    window: Option<Window>,
    selecting_area: bool,
    start_pos: Option<PhysicalPosition<f64>>,
    end_pos: Option<PhysicalPosition<f64>>,
    selected_area: Option<(f64, f64, f64, f64)>, // (x, y, width, height)
}

impl Default for WindowPortion {
    fn default() -> Self {
        Self {
            window: None,
            selecting_area: false,
            start_pos: None,
            end_pos: None,
            selected_area: None,
        }
    }
}

impl WindowPortion {
    fn run_selection(&mut self) {
        let event_loop = match EventLoop::new() {
            Ok(event_loop) => event_loop,
            Err(e) => {
                eprintln!("Errore durante la creazione dell'EventLoop: {}", e);
                return;
            }
        };
        let window = Window::new(&event_loop).unwrap();
        self.window = Some(window);

        event_loop.run_return(|event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Wait;
                    },

                    WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                        if let Some(window) = &self.window {
                            self.start_pos = Some(window.inner_position().unwrap());
                            self.selecting_area = true;
                        }
                    }
                    WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                        if self.selecting_area {
                            if let (Some(start), Some(end)) = (self.start_pos, self.end_pos) {
                                let x = start.x.min(end.x);
                                let y = start.y.min(end.y);
                                let width = (start.x - end.x).abs();
                                let height = (start.y - end.y).abs();
                                self.selected_area = Some((x, y, width, height));
                                self.selecting_area = false;
                            }
                        }
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        if self.selecting_area {
                            self.end_pos = Some(PhysicalPosition { x: position.x as f64, y: position.y as f64 });
                        }
                    }
                    _ => (),
                },
                _ => (),
            }
        });
    }
}
*/