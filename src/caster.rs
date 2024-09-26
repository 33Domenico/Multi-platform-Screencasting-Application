use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use eframe::epaint::Rect;
use scrap::{Capturer, Display, Frame};
use image::{ImageBuffer, RgbImage, DynamicImage};
use tokio::io::AsyncWriteExt;
use tokio::time::{sleep, Duration};

async fn compress_frame_to_jpeg(frame: &[u8], width: usize, height: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut img_buffer: RgbImage = ImageBuffer::new(width as u32, height as u32);
    for (i, pixel) in img_buffer.pixels_mut().enumerate() {
        let idx = i * 4;
        *pixel = image::Rgb([frame[idx + 2], frame[idx + 1], frame[idx]]); // BGRA -> RGB
    }
    let img = DynamicImage::ImageRgb8(img_buffer);
    let mut jpeg_data = Vec::new();
    {
        let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut jpeg_data);
        encoder.encode_image(&img)?;
    }
    Ok(jpeg_data)
}

fn crop_frame_to_area(
    frame: &[u8],
    frame_width: usize,
    frame_height: usize,
    area: Rect
) -> Vec<u8> {
    let (x0, y0) = (area.min.x as usize, area.min.y as usize);
    let (x1, y1) = (area.max.x as usize, area.max.y as usize);

    let cropped_width = x1 - x0;
    let cropped_height = y1 - y0;

    let mut cropped_frame = Vec::new();
    for y in y0..y1 {
        let start_idx = (y * frame_width + x0) * 4;  // BGRA
        let end_idx = start_idx + (cropped_width * 4);
        cropped_frame.extend_from_slice(&frame[start_idx..end_idx]);
    }

    cropped_frame
}

async fn capture_screen(sender: &broadcast::Sender<Vec<u8>>, stop_signal: Arc<AtomicBool>, selected_area: Option<Rect>) -> Result<(), Box<dyn Error>> {
    let display = Display::primary()?;
    let mut capturer = Capturer::new(display)?;

    while !stop_signal.load(Ordering::SeqCst) {
        let width = capturer.width();
        let height = capturer.height();

        match capturer.frame() {
            Ok(frame) => {
                println!("Frame catturato con successo, compressione in corso...");

                let selected_frame = if let Some(area) = selected_area {
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
                    (frame.to_vec(), width, height)  // No cropping if no area is selected
                };

                let (cropped_frame, cropped_width, cropped_height) = selected_frame;

                let jpeg_frame = compress_frame_to_jpeg(&cropped_frame, cropped_width, cropped_height).await?;
                let frame_size = (jpeg_frame.len() as u32).to_be_bytes();

                println!("Dimensione frame: {} byte", jpeg_frame.len());

                if let Err(e) = sender.send([&frame_size[..], &jpeg_frame[..]].concat()) {
                    eprintln!("Errore nell'invio del frame: {}", e);
                }
                println!("Frame compresso e inviato.");
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Nessun frame disponibile, aspetta e riprova
            }
            Err(e) => {
                eprintln!("Errore nella cattura del frame: {:?}", e);
            }
        }
        sleep(Duration::from_millis(100)).await;
    }

    println!("Cattura dello schermo interrotta.");
    Ok(())
}

pub async fn start_caster(addr: &str, stop_signal: Arc<AtomicBool>, selected_area: Option<Rect>) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(addr).await?;
    let (tx, _rx) = broadcast::channel::<Vec<u8>>(10);
    let tx = Arc::new(tx);

    println!("Caster avviato su {}", addr);

    let tx_clone = Arc::clone(&tx);
    let stop_signal_clone = Arc::clone(&stop_signal);

    tokio::spawn(async move {
        while !stop_signal_clone.load(Ordering::SeqCst) {
            if let Ok((mut socket, addr)) = listener.accept().await {
                println!("Nuova connessione da: {}", addr);
                let mut rx = tx_clone.subscribe();
                let stop_signal_client = Arc::clone(&stop_signal_clone);

                tokio::spawn(async move {
                    while !stop_signal_client.load(Ordering::SeqCst) {
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
                                eprintln!("Errore nella ricezione del frame dal canale: {}", e);
                                break;
                            }
                        }
                    }
                    println!("Connessione chiusa con {}", addr);
                });
            }
        }
        println!("Listener TCP interrotto.");
    });

    capture_screen(&*Arc::clone(&tx), stop_signal, selected_area).await?;  // Pass the selected area

    println!("Caster completamente fermato.");
    Ok(())
}
