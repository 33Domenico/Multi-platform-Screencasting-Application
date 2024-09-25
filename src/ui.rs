use eframe::{egui, Frame};
use eframe::WindowBuilder;
use crate::{caster, receiver};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use egui::{Context, Widget};
use winit::
    event_loop::EventLoop;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, Event, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{Window, WindowAttributes, WindowId};


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
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Screencast Application");

            ui.horizontal(|ui| {
                if ui.button("Caster").clicked() {
                    self.mode = Some(Modality::Caster);
                    self.caster_running = false;
                    self.receiver_running = false;
                    self.stop_signal.store(false, Ordering::SeqCst);
                    self.status_message = "Modalità selezionata: Caster".to_string();
                }
                if ui.button("Receiver").clicked() {
                    self.mode = Some(Modality::Receiver);
                    self.caster_running = false;
                    self.receiver_running = false;
                    self.stop_signal.store(false, Ordering::SeqCst);
                    self.status_message = "Modalità selezionata: Receiver".to_string();

                }
            });

            let mut wp = WindowPortion::default();
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

                            // Selezione area di schermo
                            if ui.button("Seleziona area").clicked() {

                                wp.selecting_area = true;
                                self.status_message = "Clicca e trascina per selezionare l'area".to_string();
                                let event_loop = EventLoop::new().unwrap();


                                event_loop.run_app(&mut wp);

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

            if let Some((x, y, width, height)) = wp.selected_area {
                ui.label(format!("Area selezionata: ({}, {}, {}, {})", x, y, width, height));
            }

            ui.label(&self.status_message);
        });
    }
}

pub struct WindowPortion{
    window: Option<Window>,
    selecting_area: bool,
    start_pos: Option<PhysicalPosition<f64>>,
    end_pos: Option<PhysicalPosition<f64>>,
    selected_area: Option<(f64, f64, f64, f64)>, // (x, y, width, height)
    status_message: String
}

impl Default for WindowPortion {
    fn default() -> Self {
        Self {
            window: None,
            selecting_area: false,
            start_pos: None,
            end_pos: None,
            selected_area: None,
            status_message: String::new()
        }
    }
}


impl ApplicationHandler for WindowPortion{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        println!("Sto creando la finestra");
        self.window = Some(event_loop.create_window(WindowAttributes::default()).unwrap());
        println!("Finestra creata");

    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.selecting_area = false;  // Imposta `running` a false per uscire
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                if let Some(position) = self.window.as_ref().unwrap().current_monitor().map(|m| m.position()) {
                    self.start_pos = Some(PhysicalPosition {
                        x: position.x as f64,
                        y: position.y as f64,
                    });
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
                        self.status_message = format!("Area selezionata: ({}, {}, {}, {})", x, y, width, height);
                    }
                    self.selecting_area = false;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if self.selecting_area {
                    self.end_pos = Some(PhysicalPosition {
                        x: position.x as f64,
                        y: position.y as f64,
                    });
                    if let Some(window) = self.window.as_ref() {
                        window.request_redraw();
                    }
                }
            }

            /**
            // Event::MainEventsCleared => {
                // Disegna il rettangolo di selezione qui
            }**/
            _ => (),
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        todo!()
    }
}

