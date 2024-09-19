use eframe::egui;
use eframe::WindowBuilder;
use crate::{caster, receiver};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    dpi::PhysicalPosition,
    platform::run_return::EventLoopExtRunReturn, // per usare `run_return`
};

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
    selecting_area: bool,
    start_pos: Option<PhysicalPosition<f64>>,
    end_pos: Option<PhysicalPosition<f64>>,
    selected_area: Option<(f64, f64, f64, f64)>, // (x, y, width, height)
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
            selecting_area: false,
            start_pos: None,
            end_pos: None,
            selected_area: None,
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

                            if ui.button("Seleziona area").clicked() {
                                self.selecting_area = true;
                                self.status_message = "Clicca e trascina per selezionare l'area".to_string();

                                let event_loop = EventLoop::<()>::new(); // Aggiunto tipo esplicito
                                let screen_rect = ctx.input(|i| i.screen_rect()); // Corretto per accettare closure
                                let width = screen_rect.width();
                                let height = screen_rect.height();

                                let window = WindowBuilder::new()
                                    .with_transparent(true)
                                    .with_decorations(false)
                                    .with_inner_size(winit::dpi::LogicalSize::new(width as f64, height as f64))
                                    .build(&event_loop)
                                    .unwrap();

                                let mut start_pos = None;
                                let mut end_pos = None;
                                let mut selecting = false;

                                event_loop.run_return(|event, _, control_flow| { // Usato run_return invece di run
                                    *control_flow = ControlFlow::Wait;

                                    match event {
                                        Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                                            *control_flow = ControlFlow::Exit; // Modificato ExitWithCode
                                        }
                                        Event::WindowEvent { event: WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. }, .. } => {
                                            if let Some(position) = window.current_monitor().map(|m| m.position()) { // Corretto metodo
                                                start_pos = Some(position);
                                                selecting = true;
                                            }
                                        }
                                        Event::WindowEvent { event: WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. }, .. } => {
                                            if selecting {
                                                if let (Some(start), Some(end)) = (start_pos, end_pos) {
                                                    let x = start.x.min(end.x);
                                                    let y = start.y.min(end.y);
                                                    let width = (start.x - end.x).abs();
                                                    let height = (start.y - end.y).abs();
                                                    self.selected_area = Some((x, y, width, height));
                                                    self.selecting_area = false;
                                                    self.status_message = format!("Area selezionata: ({}, {}, {}, {})", x, y, width, height);
                                                }
                                                selecting = false;
                                                *control_flow = ControlFlow::Exit; // Modificato ExitWithCode
                                            }
                                        }
                                        Event::WindowEvent { event: WindowEvent::CursorMoved { position, .. }, .. } => {
                                            if selecting {
                                                end_pos = Some(position);
                                                window.request_redraw();
                                            }
                                        }
                                        Event::MainEventsCleared => { // Aggiunto per gestire il redraw
                                            // Qui puoi disegnare il rettangolo di selezione
                                            println!("Ridisegnando il rettangolo di selezione");
                                        }
                                        _ => (),
                                    }
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

            if let Some((x, y, width, height)) = self.selected_area {
                ui.label(format!("Area selezionata: ({}, {}, {}, {})", x, y, width, height));
            }

            ui.label(&self.status_message);
        });
    }
}
