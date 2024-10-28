use eframe::{egui, App, Frame, CreationContext};
use crate::{caster, receiver};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}, RwLock};
use eframe::egui::{Rect, Pos2, Color32, UiBuilder, Image, Widget};
use tokio::runtime::Runtime;
use image::{ImageBuffer, Rgba};
use scrap::{Capturer, Display};
use std::time::Duration;
use std::thread;

#[derive(Debug, Clone)]
enum Modality {
    Caster,
    Receiver,
}

pub struct MyApp {
    mode: Option<Modality>,
    caster_address: String,
    status_message: String,
    caster_running: Arc<AtomicBool>,
    receiver_running: Arc<AtomicBool>,
    stop_signal: Arc<AtomicBool>,
    start_pos: Option<Pos2>,
    selecting_area: bool,
    selected_area: Option<Rect>,
    screenshot: Option<egui::TextureHandle>,
    error_message: Arc<RwLock<Option<String>>>,
    is_error: Arc<AtomicBool>,
    available_displays: Vec<DisplayInfo>,
    selected_display_index: Option<usize>,
    start_pos_relative: Option<Pos2>,  // Per salvare la posizione di inizio relativa all'immagine
}
#[derive(Clone)]
struct DisplayInfo {
    name: String,
    width: usize,
    height: usize,
    index: usize,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            mode: None,
            caster_address: String::from(""),
            status_message: String::from("Seleziona una modalità per iniziare."),
            caster_running: Arc::new(AtomicBool::new(false)),
            receiver_running:  Arc::new(AtomicBool::new(false)),
            stop_signal: Arc::new(AtomicBool::new(false)),
            start_pos: None,
            selecting_area: false,
            selected_area: None,
            screenshot: None,
            error_message: Arc::new(RwLock::new(None)),
            is_error: Arc::new(AtomicBool::new(false)),
            available_displays: Vec::new(),
            selected_display_index: None,
            start_pos_relative: None,
        }
    }
}

impl MyApp {
    pub fn new(_cc: &CreationContext<'_>) -> Self {
        let mut app = Self::default();
        app.refresh_displays();
        app
    }
    fn refresh_displays(&mut self) {
        self.available_displays.clear();
        if let Ok(displays) = Display::all() {
            for (index, display) in displays.iter().enumerate() {
                self.available_displays.push(DisplayInfo {
                    name: format!("Display {} ({}x{})", index + 1, display.width(), display.height()),
                    width: display.width(),
                    height: display.height(),
                    index,
                });
            }
        }
        // Se c'è solo un display, selezionalo automaticamente
        if self.available_displays.len() == 1 {
            self.selected_display_index = Some(0);
        }
    }

    fn display_error(&self, ui: &mut egui::Ui) {
        if self.is_error.load(Ordering::SeqCst) {
            if let Some(error) = self.error_message.read().unwrap().as_ref() {
                ui.label(egui::RichText::new(error).color(egui::Color32::RED));
            }
        }
    }
    fn clear_error(&self) {
        *self.error_message.write().unwrap() = None;
        self.is_error.store(false, Ordering::SeqCst);
    }

    fn set_error(&self, error: String) {
        *self.error_message.write().unwrap() = Some(error);
        self.is_error.store(true, Ordering::SeqCst);
    }
    fn handle_selection(&mut self, ctx: &egui::Context, image_rect: egui::Rect) {
        if self.selecting_area {

            ctx.output_mut(|o| o.cursor_icon = egui::CursorIcon::Crosshair);

            let response = ctx.input(|i| {
                let pos = i.pointer.hover_pos();
                let pressed = i.pointer.primary_pressed();
                let released = i.pointer.primary_released();
                (pos, pressed, released)
            });

            if let (Some(current_pos), pressed, released) = response {
                let clamped_pos = Pos2::new(
                    current_pos.x.clamp(image_rect.min.x, image_rect.max.x),
                    current_pos.y.clamp(image_rect.min.y, image_rect.max.y)
                );

                if pressed && self.start_pos.is_none() {
                    self.start_pos = Some(clamped_pos);
                    self.start_pos_relative = Some(Pos2::new(
                        (clamped_pos.x - image_rect.min.x) / image_rect.width(),
                        (clamped_pos.y - image_rect.min.y) / image_rect.height()
                    ));
                } else if released && self.start_pos.is_some() {
                    let start_relative = self.start_pos_relative.unwrap();
                    let end_relative = Pos2::new(
                        (clamped_pos.x - image_rect.min.x) / image_rect.width(),
                        (clamped_pos.y - image_rect.min.y) / image_rect.height()
                    );

                    if let Some(display_index) = self.selected_display_index {
                        if let Ok(displays) = Display::all() {
                            if let Some(display) = displays.get(display_index) {
                                // Calcola le coordinate rispetto al monitor selezionato
                                let screen_width = display.width() as f32;
                                let screen_height = display.height() as f32;

                                let screen_rect = Rect::from_two_pos(
                                    Pos2::new(
                                        (start_relative.x * screen_width).round(),
                                        (start_relative.y * screen_height).round()
                                    ),
                                    Pos2::new(
                                        (end_relative.x * screen_width).round(),
                                        (end_relative.y * screen_height).round()
                                    )
                                );

                                self.selected_area = Some(Rect::from_two_pos(
                                    Pos2::new(
                                        screen_rect.min.x.min(screen_rect.max.x),
                                        screen_rect.min.y.min(screen_rect.max.y)
                                    ),
                                    Pos2::new(
                                        screen_rect.min.x.max(screen_rect.max.x),
                                        screen_rect.min.y.max(screen_rect.max.y)
                                    )
                                ));
                            }
                        }
                    }

                    self.selecting_area = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                    // Ripristina la posizione della finestra

                    self.status_message = format!("Area selezionata: {:?}", self.selected_area.unwrap());
                }

                if self.selecting_area && self.start_pos.is_some() {
                    ctx.request_repaint();
                }
            }
        }


        else {
            ctx.output_mut(|o| o.cursor_icon = egui::CursorIcon::Default);
        }
    }


    fn capture_screenshot(&mut self, ctx: &egui::Context) {
        let display_index = match self.selected_display_index {
            Some(index) => index,
            None => {
                self.set_error("Nessun display selezionato".to_string());
                return;
            }
        };

        let displays = match Display::all() {
            Ok(displays) => displays,
            Err(e) => {
                self.set_error(format!("Errore nell'accesso ai display: {}", e));
                return;
            }
        };

        if display_index >= displays.len() {
            self.set_error("Indice del display non valido".to_string());
            return;
        }

        // Ottieni il display selezionato
        let target_display = displays.into_iter().nth(display_index).unwrap();
        let width= target_display.width();
        let height =target_display.height();

        let mut capturer = match Capturer::new(target_display) {
            Ok(capturer) => capturer,
            Err(e) => {
                self.set_error(format!("Errore nella creazione del capturer: {}", e));
                return;
            }
        };

        let frame = loop {
            match capturer.frame() {
                Ok(frame) => break frame,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(100));
                    continue;
                }
                Err(e) => {
                    eprintln!("Errore nella cattura del frame: {}", e);
                    return;
                }
            }
        };

        // Crea il buffer dell'immagine con le dimensioni del display selezionato
        let mut img_buffer: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(
           width as u32,
           height as u32
        );

        // Converti i pixel dal formato BGRA a RGBA
        for (i, pixel) in img_buffer.pixels_mut().enumerate() {
            let idx = i * 4;
            if idx + 3 < frame.len() {
                *pixel = Rgba([frame[idx + 2], frame[idx + 1], frame[idx], 255]);
            }
        }

        // Converti l'immagine per egui
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [width, height],
            &img_buffer.into_raw(),
        );

        self.screenshot = Some(ctx.load_texture(
            "screenshot",
            color_image,
            egui::TextureOptions::LINEAR,
        ));

        // Calcola la posizione corretta per la finestra
        if let Ok(displays) = Display::all() {
            let mut x_offset = 0;
            let mut found_display = false;

            // Calcola l'offset corretto basato sulla posizione relativa dei monitor
            for (idx, d) in displays.iter().enumerate() {
                if idx == display_index {
                    found_display = true;
                    break;
                }
                x_offset += d.width() as i32;
            }



        }

        // Imposta la modalità fullscreen
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
    }

}

impl App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut Frame) {
        if self.selected_display_index==None{
            self.refresh_displays()
        }

        if self.selecting_area {
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(Color32::from_rgba_unmultiplied(0, 0, 0, 200)))
                .show(ctx, |ui| {
                    let mut image_rect = egui::Rect::NOTHING;

                    if let Some(texture) = &self.screenshot {
                        // Calcola le dimensioni disponibili mantenendo l'aspect ratio
                        let available_size = ui.available_size();
                        let texture_size = texture.size();
                        let texture_aspect = texture_size[0] as f32 / texture_size[1] as f32;
                        let available_aspect = available_size.x / available_size.y;

                        let size = if texture_aspect > available_aspect {
                            egui::vec2(available_size.x, available_size.x / texture_aspect)
                        } else {
                            egui::vec2(available_size.y * texture_aspect, available_size.y)
                        };

                        // Calcola il rettangolo centrato per l'immagine
                        let available_rect = ui.available_rect_before_wrap();
                        image_rect = egui::Rect::from_center_size(
                            available_rect.center(),
                            size
                        );

                        let image = Image::from_texture(texture)
                            .fit_to_exact_size(size)
                            .tint(Color32::from_rgba_unmultiplied(110, 110, 110, 200));

                        ui.allocate_ui_at_rect(image_rect, |ui| {
                            image.ui(ui);
                        });

                        // Disegna il rettangolo di selezione
                        if let Some(start) = self.start_pos {
                            if let Some(current) = ui.input(|i| i.pointer.hover_pos()) {
                                let clamped_current = Pos2::new(
                                    current.x.clamp(image_rect.min.x, image_rect.max.x),
                                    current.y.clamp(image_rect.min.y, image_rect.max.y)
                                );

                                let rect = Rect::from_two_pos(start, clamped_current);
                                ui.painter().rect_stroke(rect, 0.0, (2.0, Color32::WHITE));
                                ui.painter().rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 50));
                            }
                        }
                    }

                    // Mostra il messaggio di istruzione
                    let screen_rect = ui.max_rect();
                    let center_x = screen_rect.center().x;
                    let center_y = screen_rect.center().y;
                    let rect = Rect::from_center_size(
                        Pos2::new(center_x, center_y),
                        egui::vec2(200.0, 50.0)
                    );

                    ui.allocate_new_ui(UiBuilder::max_rect(Default::default(), rect), |ui| {
                        ui.with_layout(egui::Layout::centered_and_justified(egui::Direction::TopDown), |ui| {
                            ui.colored_label(Color32::WHITE, egui::RichText::new("Clicca e trascina per selezionare l'area").strong());
                        });
                    });

                    // Gestisci la selezione con il rettangolo dell'immagine
                    self.handle_selection(ctx, image_rect);
                });
        } else {
            egui::CentralPanel::default().show(ctx, |ui| {
                self.display_error(ui);
                ui.heading("Screencast Application");
                ui.horizontal(|ui| {
                    if ui.button("Caster").clicked()  {
                        self.clear_error();
                        self.mode = Some(Modality::Caster);
                        self.stop_signal.store(false, Ordering::SeqCst);
                        self.selecting_area = false;
                        self.status_message = "Modalità selezionata: Caster".to_string();
                    }
                    if ui.button("Receiver").clicked()  {
                        self.clear_error();
                        self.mode = Some(Modality::Receiver);
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
                            ui.horizontal(|ui| {
                                ui.label("Seleziona Monitor:");
                                egui::ComboBox::from_label("")
                                    .selected_text(match self.selected_display_index {
                                        Some(index) => &self.available_displays[index].name,
                                        None => "Seleziona un monitor",
                                    })
                                    .show_ui(ui, |ui| {
                                        for (index, display) in self.available_displays.iter().enumerate() {
                                            ui.selectable_value(
                                                &mut self.selected_display_index,
                                                Some(index),
                                                &display.name,
                                            );
                                        }
                                    });

                                if ui.button("🔄").clicked() {
                                    self.refresh_displays();
                                }
                            });

                            // Visualizza l'area selezionata se presente
                            if let Some(area) = self.selected_area {
                                ui.label(format!(
                                    "Area selezionata: ({}, {}) - ({}, {})",
                                    area.min.x as i32,
                                    area.min.y as i32,
                                    area.max.x as i32,
                                    area.max.y as i32
                                ));
                            }

                            if !self.caster_running.load(Ordering::SeqCst) {
                                self.status_message="Modalità selezionata: Caster".to_string();

                                // Disabilita il pulsante "Seleziona area" se non è stato selezionato uno schermo
                                let select_area_button = ui.add_enabled(
                                    self.selected_display_index.is_some(),
                                    egui::Button::new("Seleziona area")
                                );

                                if select_area_button.clicked() {
                                    self.capture_screenshot(ctx);
                                    self.selecting_area = true;
                                    self.start_pos = None;
                                    self.status_message = "Clicca e trascina per selezionare l'area".to_string();
                                }

                                if ui.button("Avvia").clicked() {
                                    self.clear_error();
                                    self.status_message = "Trasmissione in corso...".to_string();
                                    self.caster_running.store(true,Ordering::SeqCst);
                                    self.stop_signal.store(false, Ordering::SeqCst);

                                    let stop_signal = self.stop_signal.clone();
                                    let ctx = ctx.clone();
                                    let selected_area = self.selected_area;
                                    let caster_address = self.caster_address.clone();
                                    let error_message = self.error_message.clone();
                                    let is_error = self.is_error.clone();
                                    let is_running = self.caster_running.clone();
                                    let selected_display_index = self.selected_display_index.unwrap_or_else(|| 0);

                                    std::thread::spawn(move || {
                                        Runtime::new().unwrap().block_on(async {
                                            if let Err(e) = caster::start_caster(&caster_address, stop_signal, selected_area,selected_display_index).await {
                                                let error = format!("Errore nel caster: {}", e);
                                                *error_message.write().unwrap() = Some(error);
                                                is_error.store(true, Ordering::SeqCst);
                                                eprintln!("Errore: {}", e);
                                            }
                                            is_running.store(false, Ordering::SeqCst);
                                        });
                                        ctx.request_repaint();
                                    });
                                }
                            } else {
                                if ui.button("Stop").clicked() {
                                    self.status_message = "Interrompendo il caster...".to_string();
                                    self.stop_signal.store(true, Ordering::SeqCst);
                                    self.caster_running.store(false,Ordering::SeqCst);
                                    self.status_message = "Caster interrotto.".to_string();
                                }

                                ui.label(egui::RichText::new("\nShortcuts:\nFn + F1 --> Metti in pausa lo stream;\nFn + F2 --> Blank screen;\nESC --> Interrompi lo stream\n")
                                    .color(egui::Color32::BLACK));
                            }
                        }
                        Modality::Receiver => {
                            ui.horizontal(|ui| {
                                ui.label("Indirizzo caster: es.127.0.0.1:12345 in locale o tra più dispositivi 192.168.165.219:8080");
                                ui.text_edit_singleline(&mut self.caster_address);
                            });

                            if !self.receiver_running.load(Ordering::SeqCst) {
                                self.status_message="Modalità selezionata: Receiver".to_string();
                                if ui.button("Avvia").clicked()   {
                                    self.clear_error();
                                    let addr = self.caster_address.clone();
                                    self.status_message = "Connettendo al caster...".to_string();
                                    self.receiver_running.store(true,Ordering::SeqCst) ;
                                    self.stop_signal.store(false, Ordering::SeqCst);

                                    let stop_signal = self.stop_signal.clone();
                                    let ctx = ctx.clone();
                                    let error_message = self.error_message.clone();
                                    let is_error = self.is_error.clone();
                                    let is_running=self.receiver_running.clone();
                                    std::thread::spawn(move || {
                                        Runtime::new().unwrap().block_on(async {
                                            if let Err(e) = receiver::receive_frame(&addr, stop_signal).await {
                                                let error = format!("Errore nel caster: {}", e);
                                                *error_message.write().unwrap() = Some(error);
                                                is_error.store(true, Ordering::SeqCst);
                                                eprintln!("Errore: {}", e);
                                            }
                                                is_running.store(false,Ordering::SeqCst);

                                        });
                                        ctx.request_repaint();
                                    });
                                }
                            } else {
                                if ui.button("Stop").clicked() {
                                    self.status_message = "Interrompendo il receiver...".to_string();
                                    self.stop_signal.store(true, Ordering::SeqCst);
                                    self.receiver_running.store(false,Ordering::SeqCst);
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