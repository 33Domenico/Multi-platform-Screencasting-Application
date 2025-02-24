use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use eframe::epaint::Rect;
use scrap::{Capturer, Display};
use image::{ImageBuffer, RgbImage, DynamicImage};
use tokio::io::AsyncWriteExt;
use tokio::time::{sleep, Duration};
use device_query::{DeviceQuery, DeviceState, Keycode};


struct HotkeyState {
    paused: Arc<AtomicBool>,
    screen_blanked: Arc<AtomicBool>,
    terminate: Arc<AtomicBool>,
}


fn handle_hotkeys(hotkey_state: Arc<HotkeyState>) {
    let device_state = DeviceState::new();
    let mut last_keys = Vec::new();

    loop {
        let keys: Vec<Keycode> = device_state.get_keys();

        if keys != last_keys {

            if keys.contains(&Keycode::F1) {
                hotkey_state.paused.fetch_xor(true, Ordering::SeqCst);
                println!("Trasmissione {}.", if hotkey_state.paused.load(Ordering::SeqCst) { "paused" } else { "resumed" });
            }
            if keys.contains(&Keycode::F2) {
                hotkey_state.screen_blanked.fetch_xor(true, Ordering::SeqCst);
                println!("Schermo {}.", if hotkey_state.screen_blanked.load(Ordering::SeqCst) { "blanked" } else { "unblanked" });
            }
            if keys.contains(&Keycode::Escape) {
                hotkey_state.terminate.store(true, Ordering::SeqCst);
                println!("Terminazione richiesta.");
                break;
            }
        }

        last_keys = keys;
        std::thread::sleep(Duration::from_millis(100));
    }
}

async fn compress_frame_to_jpeg(frame: &[u8], width: usize, height: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut img_buffer: RgbImage = ImageBuffer::new(width as u32, height as u32);
    for (i, pixel) in img_buffer.pixels_mut().enumerate() {
        let idx = i * 4;
        *pixel = image::Rgb([frame[idx + 2], frame[idx + 1], frame[idx]]);
    }
    let img = DynamicImage::ImageRgb8(img_buffer);
    let mut jpeg_data = Vec::new();
    {
        let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut jpeg_data);
        encoder.encode_image(&img)?;
    }
    Ok(jpeg_data)
}

async fn capture_screen(
    sender: &broadcast::Sender<Vec<u8>>,
    stop_signal: Arc<AtomicBool>,
    selected_area: Option<Rect>,
    hotkey_state: Arc<HotkeyState>,
    display_index: usize
) -> Result<(), Box<dyn Error>> {
    let displays = Display::all()?;
    if display_index >= displays.len() {
        return Err("Indice del display non valido".into());
    }
    let display = displays.into_iter().nth(display_index).unwrap();
    let mut capturer = Capturer::new(display)?;
    let mut last_frame: Option<Vec<u8>> = None;
    while !stop_signal.load(Ordering::SeqCst) && !hotkey_state.terminate.load(Ordering::SeqCst) {
        if hotkey_state.paused.load(Ordering::SeqCst) {
            sleep(Duration::from_millis(100)).await;
            continue;
        }
        let width = capturer.width();
        let height = capturer.height();
        let default_width = width;
        let default_height = height;
        match capturer.frame() {
            Ok(frame) => {
                println!("Frame catturato con successo, compressione in corso...");
                let (selected_frame, cropped_width, cropped_height) = if let Some(area) = selected_area {
                    let start_x = area.min.x as usize;
                    let start_y = area.min.y as usize;
                    let end_x = area.max.x as usize;
                    let end_y = area.max.y as usize;
                    let mut cropped_frame = Vec::new();
                    for y in start_y..end_y {
                        let start_index = y * width * 4 + start_x * 4;
                        let end_index = y * width * 4 + end_x * 4;
                        cropped_frame.extend_from_slice(&frame[start_index..end_index]);
                    }
                    (cropped_frame, end_x - start_x, end_y - start_y)
                } else {
                    (frame.to_vec(), width, height)
                };
                let jpeg_frame = if hotkey_state.screen_blanked.load(Ordering::SeqCst) {
                    let blank_frame = vec![0; cropped_width * cropped_height * 4];
                    compress_frame_to_jpeg(&blank_frame, cropped_width, cropped_height).await?
                } else {
                    compress_frame_to_jpeg(&selected_frame, cropped_width, cropped_height).await?
                };
                last_frame = Some(jpeg_frame.clone());
                let frame_size = (jpeg_frame.len() as u32).to_be_bytes();
                if let Err(e) = sender.send([&frame_size[..], &jpeg_frame[..]].concat()) {
                    eprintln!("Errore nell'invio del frame: {}", e);
                }
            },
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if hotkey_state.screen_blanked.load(Ordering::SeqCst) {
                    let jpeg_frame = if let Some(ref frame) = last_frame {
                        frame.clone()
                    } else {
                        let blank_frame = vec![0; default_width * default_height * 4];
                        compress_frame_to_jpeg(&blank_frame, default_width, default_height).await?
                    };
                    let frame_size = (jpeg_frame.len() as u32).to_be_bytes();
                    if let Err(e) = sender.send([&frame_size[..], &jpeg_frame[..]].concat()) {
                        eprintln!("Errore nell'invio del frame di blank screen: {}", e);
                    }
                } else if let Some(ref frame) = last_frame {
                    let frame_size = (frame.len() as u32).to_be_bytes();
                    if let Err(e) = sender.send([&frame_size[..], &frame[..]].concat()) {
                        eprintln!("Errore nell'invio del frame di heartbeat: {}", e);
                    }
                }
                sleep(Duration::from_millis(10)).await;
            },
            Err(e) => {
                eprintln!("Errore nella cattura del frame: {:?}", e);
            }
        }
        sleep(Duration::from_millis(10)).await;
    }
    println!("Cattura dello schermo interrotta.");
    Ok(())
}



pub async fn start_caster(addr: &str, stop_signal: Arc<AtomicBool>, selected_area: Option<Rect>,display_index: usize,paused: Arc<AtomicBool>, screen_blanked: Arc<AtomicBool>,terminate: Arc<AtomicBool>) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(addr).await?;
    let (tx, _rx) = broadcast::channel::<Vec<u8>>(100);
    let tx = Arc::new(tx);
    println!("Caster avviato su {}", addr);
    let hotkey_state = Arc::new(HotkeyState {
        paused,
        screen_blanked,
        terminate,
    });

    let hotkey_state_clone = Arc::clone(&hotkey_state);
    std::thread::spawn(move || {
        handle_hotkeys(hotkey_state_clone);
    });

    let tx_clone = Arc::clone(&tx);
    let stop_signal_clone = Arc::clone(&stop_signal);
    let hotkey_state_clone = Arc::clone(&hotkey_state);

    tokio::spawn(async move {
        while !stop_signal_clone.load(Ordering::SeqCst) && !hotkey_state_clone.terminate.load(Ordering::SeqCst) {
            if let Ok((mut socket, addr)) = listener.accept().await {
                println!("Nuova connessione da: {}", addr);
                let mut rx = tx_clone.subscribe();
                let stop_signal_client = Arc::clone(&stop_signal_clone);
                let hotkey_state_client = Arc::clone(&hotkey_state_clone);
                tokio::spawn(async move {
                    while !stop_signal_client.load(Ordering::SeqCst) && !hotkey_state_client.terminate.load(Ordering::SeqCst) {
                        match rx.recv().await {
                            Ok(frame) => {
                                if frame.len() < 4 {
                                    eprintln!("Errore: frame troppo piccolo per contenere la dimensione.");
                                    break;
                                }
                                let frame_size_bytes = &frame[0..4];
                                let frame_data = &frame[4..];
                                if socket.write_all(&frame_size_bytes).await.is_err() ||
                                    socket.write_all(&frame_data).await.is_err() {
                                    eprintln!("Errore nell'invio del frame al client {}", addr);
                                    break;
                                }
                            }
                            Err(e) => {
                                if e.to_string().contains("lagged") {
                                    eprintln!("Avviso: il canale è in ritardo, salto alcuni frame: {}", e);
                                    continue; // Continue instead of breaking
                                }
                                eprintln!("Errore nella ricezione del frame dal canale: {}", e);
                                break;
                            }
                        }
                    }
                    println!("Connessione chiusa con {}", addr);
                });
            }
        }

        // Invia un segnale esplicito di chiusura ai receiver
        if let Err(_) = tx_clone.send(vec![0, 0, 0, 0]) {
            eprintln!("Errore nell'invio del segnale di terminazione.");
        }

        println!("Listener TCP interrotto.");
    });

    capture_screen(&*Arc::clone(&tx), stop_signal, selected_area, Arc::clone(&hotkey_state),display_index).await?;
    println!("Caster completamente fermato.");
    hotkey_state.screen_blanked.store(false, Ordering::SeqCst);
    hotkey_state.paused.store(false, Ordering::SeqCst);
    hotkey_state.terminate.store(false, Ordering::SeqCst);
    Ok(())
}
