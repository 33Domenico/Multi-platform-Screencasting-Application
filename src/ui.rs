use std::fmt::format;
use eframe::{egui, App, Frame, CreationContext};
use crate::{caster, receiver};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}, RwLock,Mutex};
use eframe::egui::{Rect, Pos2, Color32, UiBuilder, Image, Widget, FontId};
use tokio::runtime::Runtime;
use image::{ImageBuffer, Rgba};
use scrap::{Capturer, Display};
use std::time::Duration;
use std::thread;
use crate::receiver::{ReceiverState, SharedFrame};

#[derive(Debug, Clone)]
enum Modality {
    Caster,
    Receiver,
}

#[derive(PartialEq, Default, Clone, Copy)]
enum AnnotationTool {
    #[default]
    None,
    Rectangle,
    Arrow,
    Text,
}


#[derive(PartialEq, Clone)]
struct TextAnnotation {
    pos: Pos2,
    content: String,
    is_editing: bool,
}

struct AnnotationState {
    active_tool: AnnotationTool,
    start_pos: Option<Pos2>,
    end_pos: Option<Pos2>,
    annotations: Vec<Annotation>,
    editing_text: Option<String>,
    text_edit_id: Option<egui::Id>,
}

impl Default for AnnotationState {
    fn default() -> Self {
        Self {
            active_tool: AnnotationTool::None,
            start_pos: None,
            end_pos: None,
            annotations: Vec::new(),
            editing_text: None,
            text_edit_id: None,
        }
    }
}


pub enum Annotation {
    Rectangle {
        rect: egui::Rect,
        color: Color32,
    },
    Arrow {
        start: Pos2,
        end: Pos2,
        color: Color32,
    },
    Text {
        pos: Pos2,
        content: String,
        is_editing: bool,
        color: Color32,
    },
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
    shared_frame: Arc<Mutex<SharedFrame>>,
    stream_texture: Option<egui::TextureHandle>,
    receiver_state: Arc<Mutex<ReceiverState>>, // Add this field
    annotation_state: AnnotationState, // Add this field
    annotations: Vec<Annotation>,
    toolbar_visible: bool,
    paused: Arc<AtomicBool>,
    screen_blanked: Arc<AtomicBool>,
    terminate: Arc<AtomicBool>,
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
            shared_frame: Arc::new(Mutex::new(SharedFrame::default())),
            stream_texture: None,
            receiver_state: Arc::new(Mutex::new(ReceiverState::new())),
            annotation_state: AnnotationState::default(), // Initialize this field
            annotations: Vec::new(),
            toolbar_visible: false,
            paused: Arc::new(AtomicBool::new(false)),
            screen_blanked: Arc::new(AtomicBool::new(false)),
            terminate: Arc::new(AtomicBool::new(false))
        }
    }
}

impl MyApp {
    pub fn new(_cc: &CreationContext<'_>) -> Self {
        let mut app = Self::default();
        app.refresh_displays();
        app
    }
    fn handle_recording_error(&self, error: String) {
        self.set_error(format!("Errore di registrazione: {}", error));
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

                // Add safety checks for the image_rect dimensions
                if image_rect.width() <= 0.0 || image_rect.height() <= 0.0 {
                    self.selecting_area = false;
                    self.set_error("Invalid display dimensions".to_string());
                    return;
                }

                let clamped_pos = Pos2::new(
                    current_pos.x.clamp(image_rect.min.x, image_rect.max.x),
                    current_pos.y.clamp(image_rect.min.y, image_rect.max.y)
                );

                if pressed && self.start_pos.is_none() {
                    // Add validation for the starting position
                    if clamped_pos.x.is_finite() && clamped_pos.y.is_finite() {
                        self.start_pos = Some(clamped_pos);
                        self.start_pos_relative = Some(Pos2::new(
                            (clamped_pos.x - image_rect.min.x) / image_rect.width(),
                            (clamped_pos.y - image_rect.min.y) / image_rect.height()
                        ));
                    }
                } else if released && self.start_pos.is_some() {
                    if let Some(start_relative) = self.start_pos_relative {
                        // Validate relative coordinates
                        let end_relative = Pos2::new(
                            ((clamped_pos.x - image_rect.min.x) / image_rect.width()).clamp(0.0, 1.0),
                            ((clamped_pos.y - image_rect.min.y) / image_rect.height()).clamp(0.0, 1.0)
                        );
                        if let Some(display_index) = self.selected_display_index {
                            if let Ok(displays) = Display::all() {
                                if let Some(display) = displays.get(display_index) {
                                    let screen_width = display.width() as f32;
                                    let screen_height = display.height() as f32;

                                    // Ensure we're creating a valid rectangle
                                    let min_x = (start_relative.x.min(end_relative.x) * screen_width).round();
                                    let min_y = (start_relative.y.min(end_relative.y) * screen_height).round();
                                    let max_x = (start_relative.x.max(end_relative.x) * screen_width).round();
                                    let max_y = (start_relative.y.max(end_relative.y) * screen_height).round();
                                    // Additional validation before creating the rectangle
                                    if min_x < max_x && min_y < max_y {
                                        self.selected_area = Some(Rect::from_min_max(
                                            Pos2::new(min_x, min_y),
                                            Pos2::new(max_x, max_y)
                                        ));
                                    }
                                }
                            }
                        }

                        self.selecting_area = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));

                        if let Some(area) = self.selected_area {
                            self.status_message = format!("Area selezionata: {:?}", area);
                        } else {
                            self.status_message = "Selezione area non valida".to_string();
                        }
                    }
                }

                if self.selecting_area && self.start_pos.is_some() {
                    ctx.request_repaint();
                }
            }
        } else {
            ctx.output_mut(|o| o.cursor_icon = egui::CursorIcon::Default);
        }
    }

    fn draw_arrow(painter: &egui::Painter, start: egui::Pos2, end: egui::Pos2, color: egui::Color32) {
        let stroke = egui::Stroke::new(2.0, color);

        // Calcola la direzione e la lunghezza della freccia
        let dir = end - start;
        let length = dir.length();
        if length < 5.0 {
            return; // Evita di disegnare frecce troppo piccole
        }

        // Normalizza la direzione
        let dir_normalized = dir / length;

        // **Ingrandire la punta della freccia**
        let arrowhead_length = 16.0; // Aumentato
        let arrowhead_width = 10.0; // Aumentato

        // Calcola la base della punta della freccia
        let tip = end;
        let base = end - dir_normalized * arrowhead_length;

        // Calcola i punti laterali della punta
        let perp = egui::Vec2::new(-dir_normalized.y, dir_normalized.x) * (arrowhead_width / 2.0);
        let left = base + perp;
        let right = base - perp;

        // Disegna il corpo della freccia (senza la punta)
        let line_end = base; // La linea finisce alla base della punta
        painter.line_segment([start, line_end], stroke);

        // Disegna la punta della freccia come triangolo
        let points = vec![tip, left, right];
        painter.add(egui::Shape::convex_polygon(points, color, stroke));
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
            Ok(displays) => {
                displays },
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
            Ok(capturer) => {
                capturer },
            Err(e) => {
                self.set_error(format!("Errore nella creazione del capturer: {}", e));
                return;
            }
        };
        let frame = loop {
            match capturer.frame() {
                Ok(frame) => {
                    break frame; },
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
        // Imposta la modalità fullscreen
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Transparent(false));
    }
    fn show_annotation_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let mut tool_button = |ui: &mut egui::Ui, tool: AnnotationTool, icon: &str ,label: &str| {
                let button = egui::Button::new(format!("{icon} {label}"))
                    .min_size(egui::vec2(40.0, 20.0));
                let response = ui.add(button);

                if response.clicked(){
                    self.annotation_state.active_tool = tool.clone();
                }

                if self.annotation_state.active_tool == tool{
                    response.clone().highlight();
                }
                response.on_hover_text(format!("Usa lo strumento: {label}"));
            };

            tool_button(ui, AnnotationTool::Rectangle, "▭", "Rettangolo");
            tool_button(ui, AnnotationTool::Arrow, "➡", "Freccia");
            tool_button(ui, AnnotationTool::Text, "📝", "Testo");
            // Clear button
            let clear_button = egui::Button::new("❌ Cancella Tutto")
                .min_size(egui::vec2(100.0, 40.0));

            if ui.add(clear_button).on_hover_text("Elimina tutte le annotazioni").clicked(){
                self.annotation_state.annotations.clear();
            }
        });
    }

    fn handle_annotations(&mut self, ui: &mut egui::Ui) {
        let pointer_pos = ui.input(|i| i.pointer.hover_pos());
        let mouse_pressed = ui.input(|i| i.pointer.primary_pressed());
        let mouse_released = ui.input(|i| i.pointer.primary_released());

        if let Some(pos) = pointer_pos {
            if mouse_pressed {
                self.annotation_state.start_pos = Some(pos);
                if self.annotation_state.active_tool == AnnotationTool::Text
                    && self.annotation_state.editing_text.is_none() {
                    self.annotation_state.editing_text = Some(String::new());
                    self.annotation_state.text_edit_id = Some(egui::Id::new("text_edit"));
                }
            } else if mouse_released {
                if let Some(start) = self.annotation_state.start_pos {
                    match self.annotation_state.active_tool {
                        AnnotationTool::Rectangle => {
                            let rect = Rect::from_two_pos(start, pos);
                            self.annotation_state.annotations.push(Annotation::Rectangle {
                                rect,
                                color: Color32::WHITE,
                            });
                            self.annotation_state.start_pos = None;
                            self.annotation_state.end_pos = None;
                        },
                        AnnotationTool::Arrow => {
                            self.annotation_state.annotations.push(Annotation::Arrow {
                                start,
                                end: pos,
                                color: Color32::WHITE,
                            });
                            self.annotation_state.start_pos = None;
                            self.annotation_state.end_pos = None;

                        },
                        AnnotationTool::Text => {
                            // Non resettare lo stato per il text tool
                        },
                        _ => {}
                    }
                }
            } else if mouse_pressed {
                // Aggiorna la posizione finale durante il trascinamento
                self.annotation_state.end_pos = Some(pos);
            }
        }

        // Handle text editing prima del painter
        if let Some(start) = self.annotation_state.start_pos {
            if self.annotation_state.active_tool == AnnotationTool::Text {
                if let Some(editing_text) = &mut self.annotation_state.editing_text {
                    let text_edit = egui::TextEdit::singleline(editing_text)
                        .desired_width(200.0)
                        .font(FontId::proportional(14.0));

                    let response = ui.put(
                        Rect::from_min_size(start, egui::Vec2::new(200.0, 20.0)),
                        text_edit
                    );

                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if !editing_text.is_empty() {
                            self.annotation_state.annotations.push(Annotation::Text {
                                pos: start,
                                content: editing_text.clone(),
                                is_editing: true,
                                color: Color32::WHITE,
                            });
                        }
                        self.annotation_state.editing_text = None;
                        self.annotation_state.text_edit_id = None;
                        self.annotation_state.start_pos = None;
                    }
                }
            }
        }
        // Draw existing annotations
        let painter = ui.painter();
        for annotation in &self.annotation_state.annotations {
            match annotation {
                Annotation::Rectangle{rect, ..} => {
                    painter.rect_stroke(*rect, 0.0, egui::Stroke::new(2.0, Color32::WHITE));
                },
                Annotation::Arrow { start, end, .. } => {
                    Self::draw_arrow(&painter, *start, *end, Color32::WHITE);
                },
                Annotation::Text { pos, content, .. } => {
                    painter.text(
                        *pos,
                        egui::Align2::LEFT_TOP,
                        content,
                        FontId::proportional(14.0),
                        Color32::WHITE,
                    );
                },
            }
        }


        // Draw current annotation preview
        if let (Some(start), Some(current_pos)) = (self.annotation_state.start_pos, pointer_pos) {
            match self.annotation_state.active_tool {
                AnnotationTool::Rectangle => {
                    let rect = Rect::from_two_pos(start, current_pos);
                    painter.rect_stroke(rect, 0.0, egui::Stroke::new(2.0, Color32::WHITE));
                },
                AnnotationTool::Arrow => {
                    Self::draw_arrow(&painter, start, current_pos, Color32::WHITE);
                },
                AnnotationTool::Text => {
                    // Text preview not needed as we show the text edit directly
                },
                _ => {}
            }
        }

    }


    fn save_original_window_state(&self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Transparent(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));

    }


    fn set_fullscreen_transparent(&self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Transparent(true)); // Imposta la trasparenza
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
    }

    fn get_shortcuts_message(&self) -> String {
        "\nShortcuts\n\
        Fn + F1 --> Metti in pausa lo stream\n\
        Fn + F2--> Blank screen\n\
        ESC --> Interrompi lo stream\n"
            .to_string()

    }
}

impl App for MyApp {

    fn update(&mut self, ctx: &egui::Context, frame: &mut Frame) {
        if self.selected_display_index==None{
            self.refresh_displays()
        }

        if self.selecting_area {
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(Color32::from_rgba_unmultiplied(0, 0, 0, 0)))
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
                    if self.screenshot.is_some() { // Add this condition
                        self.handle_selection(ctx, image_rect);
                    }
                });
        } else if self.toolbar_visible==true && self.caster_running.load(Ordering::SeqCst) {
                self.set_fullscreen_transparent(ctx);
                egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::from_rgba_unmultiplied(0, 0, 0, 20))).show(ctx, |ui| {

                    self.display_error(ui);
                    ui.label("Casting in corso...");
                    self.handle_annotations(ui);
                });
                // Qui disegni la tua toolbar
                egui::Window::new("Toolbar").fixed_size(egui::Vec2::new(250.0, 40.0))
                    .title_bar(false)
                    .resizable(false)
                    .open(&mut true)
                    .show(ctx, |ui| {
                        ui.label("Toolbar");
                        self.show_annotation_toolbar(ui);
                        if ui.button("⏹ Chiudi toolbar").clicked() {
                            self.status_message = "Chiudendo toolbar".to_string();
                            self.toolbar_visible = false;
                            self.save_original_window_state(ctx);
                        }
                    });

        } else {
            egui::CentralPanel::default().show(ctx, |ui| {
                self.display_error(ui);
                ui.heading("Screencast Application");
                ui.horizontal(|ui| {
                    let caster_button = ui.add_enabled(!self.receiver_running.load(Ordering::SeqCst), egui::Button::new("Caster"));
                    if caster_button.clicked()  {
                        self.clear_error();
                        self.mode = Some(Modality::Caster);
                        self.stop_signal.store(false, Ordering::SeqCst);
                        self.selecting_area = false;
                        self.status_message = "Modalità selezionata: Caster".to_string();
                    }
                    let receiver_button = ui.add_enabled(!self.caster_running.load(Ordering::SeqCst), egui::Button::new("Receiver"));
                    if receiver_button.clicked()  {
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
                                    let paused_clone=self.paused.clone();
                                    let screen_blanked_clone=self.screen_blanked.clone();
                                    let terminate_clone=self.terminate.clone();

                                    std::thread::spawn(move || {
                                        Runtime::new().unwrap().block_on(async {
                                            if let Err(e) = caster::start_caster(&caster_address, stop_signal, selected_area,selected_display_index,paused_clone,screen_blanked_clone, terminate_clone).await {
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
                                ui.label(self.get_shortcuts_message());
                                if ui.button(if self.toolbar_visible {"Hide Toolbar"} else {"Show Toolbar"}).clicked() {
                                    self.toolbar_visible = !self.toolbar_visible;
                                }
                                if ui.button("⏹").clicked() {
                                    self.status_message = "Interrompendo il caster...".to_string();
                                    self.stop_signal.store(true, Ordering::SeqCst);
                                    self.caster_running.store(false, Ordering::SeqCst);
                                    self.status_message = "Caster interrotto.".to_string();
                                }
                                // Mostra scritta gialla per pausa
                                if self.paused.load(Ordering::SeqCst) {
                                        ui.label(
                                            egui::RichText::new("PAUSA")
                                                .size(24.0) // Dimensione del testo
                                                .color(Color32::YELLOW) // Colore giallo
                                                .strong(), // Testo in grassetto
                                        );
                                }

                                // Mostra scritta gialla per blank screen
                                if self.screen_blanked.load(Ordering::SeqCst) {
                                        ui.label(
                                            egui::RichText::new("BLANK SCREEN")
                                                .size(24.0) // Dimensione del testo
                                                .color(Color32::YELLOW) // Colore giallo
                                                .strong(), // Testo in grassetto
                                        );
                                }



                            }
                        }
                        Modality::Receiver => {
                            ui.horizontal(|ui| {
                                ui.label("Indirizzo caster: es.127.0.0.1:12345 in locale o tra più dispositivi 192.168.165.219:8080");
                                ui.text_edit_singleline(&mut self.caster_address);
                            });


                            if !self.receiver_running.load(Ordering::SeqCst) {
                                self.status_message="Modalità selezionata: Receiver".to_string();
                                if ui.button("Avvia").clicked() {
                                    self.clear_error();
                                    let addr = self.caster_address.clone();
                                    let receiver_state = Arc::clone(&self.receiver_state);
                                    self.receiver_running.store(true, Ordering::SeqCst);
                                    self.stop_signal.store(false, Ordering::SeqCst);

                                    let stop_signal = self.stop_signal.clone();
                                    let ctx = ctx.clone();
                                    let error_message = self.error_message.clone();
                                    let is_error = self.is_error.clone();
                                    let is_running = self.receiver_running.clone();
                                    let shared_frame = self.shared_frame.clone();

                                    std::thread::spawn(move || {
                                        Runtime::new().unwrap().block_on(async {
                                            if let Err(e) = receiver::receive_frame(&addr, stop_signal, shared_frame,receiver_state).await {
                                                let error = format!("Errore nel receiver: {}", e);
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
                                ui.horizontal(|ui| {
                                    if let Ok(mut receiver_state) = Arc::clone(&self.receiver_state).lock() {

                                        if receiver_state.recording {
                                            if ui.add(egui::Button::new("⏹ Arresta Registrazione")
                                                .fill(Color32::from_rgb(255, 50, 50)))
                                                .clicked()
                                            {
                                                match receiver_state.stop_recording() {
                                                    Ok(_) => {
                                                        self.status_message = "Registrazione completata con successo.".to_string();
                                                        self.clear_error();
                                                    },
                                                    Err(e) => {
                                                        self.handle_recording_error(e.to_string());
                                                    }
                                                }
                                            }
                                            // Mostra stato registrazione
                                            ui.label(format!("Frame registrati: {}", receiver_state.frame_count));
                                        } else {
                                            if ui.add(egui::Button::new("⏺ Avvia Registrazione")
                                                .fill(Color32::from_rgb(50, 255, 50)))
                                                .clicked()
                                            {
                                                match std::process::Command::new("ffmpeg").arg("-version").output() {
                                                    Ok(_) => {
                                                        match receiver_state.start_recording() {
                                                            Ok(_) => {
                                                                self.status_message = "Registrazione avviata.".to_string();
                                                                self.clear_error();
                                                            },
                                                            Err(e) => {
                                                                self.handle_recording_error(e.to_string());
                                                            }
                                                        }
                                                    },
                                                    Err(_) => {
                                                        self.handle_recording_error(
                                                            "FFmpeg non trovato. Installare FFmpeg per abilitare la registrazione video."
                                                                .to_string()
                                                        );
                                                    }
                                                }

                                            }
                                        }
                                    }
                                });


                                if let Ok(mut shared) = self.shared_frame.lock() {
                                    if shared.new_frame {
                                        let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                            [shared.width, shared.height],
                                            &shared.buffer,
                                        );

                                        self.stream_texture = Some(ctx.load_texture(
                                            "stream",
                                            color_image,
                                            egui::TextureOptions::LINEAR,
                                        ));
                                        shared.new_frame = false;
                                    }
                                }

                                // Mostra un indicatore di registrazione quando è attiva
                                if let Ok(receiver_state) = self.receiver_state.lock() {
                                    if receiver_state.recording {
                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new("⚫ REC")
                                                .color(Color32::from_rgb(255, 0, 0))
                                                .strong());

                                            // Mostra il path di output
                                            ui.label(format!("Salvando in: {}", receiver_state.output_dir));
                                        });
                                    }
                                }

                                // Mostra il frame
                                if ui.button("Stop").clicked() {
                                    self.status_message = "Interrompendo il receiver...".to_string();
                                    self.stop_signal.store(true, Ordering::SeqCst);
                                    self.receiver_running.store(false,Ordering::SeqCst);
                                }
                                // Mostra il frame adattandolo alla finestra
                                if let Some(texture) = &self.stream_texture {
                                    // Ottieni le dimensioni disponibili nell'UI
                                    let available_size = ui.available_size();

                                    // Ottieni le dimensioni originali della texture
                                    let texture_size = texture.size_vec2();

                                    // Calcola il rapporto di aspetto della texture e dello spazio disponibile
                                    let texture_aspect = texture_size.x / texture_size.y;
                                    let available_aspect = available_size.x / available_size.y;

                                    // Calcola le dimensioni finali mantenendo il rapporto di aspetto
                                    let display_size = if texture_aspect > available_aspect {
                                        // La texture è più larga rispetto allo spazio disponibile
                                        egui::vec2(available_size.x, available_size.x / texture_aspect)
                                    } else {
                                        // La texture è più alta rispetto allo spazio disponibile
                                        egui::vec2(available_size.y * texture_aspect, available_size.y)
                                    };
                                    let image = Image::from_texture(texture)
                                        .fit_to_exact_size(display_size);
                                    // Crea e mostra l'immagine ridimensionata
                                    image.ui(ui);
                                }
                                ctx.request_repaint();
                            }
                        }
                    }
                }

                ui.label(&self.status_message);
            });
        }
    }
}