use tokio::net::TcpStream;
use std::io;
use tokio::io::AsyncReadExt;
use image::ImageReader;
use minifb::{Window, WindowOptions};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use tokio::time::{sleep, Duration, timeout};

pub async fn receive_frame(addr: &str, stop_signal: Arc<AtomicBool>) -> io::Result<()> {
    let mut stream = TcpStream::connect(addr).await?;
    let mut window: Option<Window> = None;
    let mut width: usize = 0;
    let mut height: usize = 0;
    let read_timeout = Duration::from_secs(2);  // Timeout di 2 secondi per leggere i dati

    while !stop_signal.load(Ordering::SeqCst) {
        let mut size_buf = [0u8; 4];

        // Usa il timeout per la lettura
        match timeout(read_timeout, stream.read_exact(&mut size_buf)).await {
            Ok(Ok((_))) => {
                let frame_size = u32::from_be_bytes(size_buf) as usize;
                println!("Ricevuto frame di dimensione: {} byte", frame_size);

                if frame_size > 10_000_000 {
                    eprintln!("Frame troppo grande: {} byte", frame_size);
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Frame troppo grande"));
                }

                let mut buffer = vec![0u8; frame_size];
                stream.read_exact(&mut buffer).await?;

                let img = ImageReader::new(std::io::Cursor::new(buffer))
                    .with_guessed_format()
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Errore nel formato dell'immagine: {}", e)))?
                    .decode()
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Errore durante la decodifica dell'immagine: {}", e)))?;

                let img = img.to_rgba8();
                let (w, h) = img.dimensions();

                if window.is_none() {
                    width = w as usize;
                    height = h as usize;
                    window = Some(Window::new(
                        "Ricezione Frame",
                        width,
                        height,
                        WindowOptions::default(),
                    ).expect("Impossibile creare la finestra!"));
                }

                if let Some(ref mut win) = window {
                    let buffer: Vec<u32> = img
                        .pixels()
                        .map(|p| {
                            let rgba = p.0;
                            let r = rgba[0] as u32;
                            let g = rgba[1] as u32;
                            let b = rgba[2] as u32;
                            let a = rgba[3] as u32;
                            (r << 16) | (g << 8) | b | (a << 24)
                        })
                        .collect();

                    if win.is_open() {
                        win.update_with_buffer(&buffer, width, height).unwrap();
                    } else {
                        break;
                    }
                }
            }
            Ok(Err(e)) => {
                eprintln!("Errore durante la lettura della dimensione del frame: {}", e);
                return Err(e);
            }
            Err(_) => {
                // Il timeout è scaduto, nessun frame ricevuto
                println!("Timeout scaduto, nessun frame ricevuto. Mantengo la finestra attiva.");

                if let Some(ref mut win) = window {
                    if win.is_open() {
                        win.update();  // Mantieni la finestra aggiornata senza nuovi frame
                    } else {
                        break;
                    }
                }

                // Aggiungi un ritardo per evitare di consumare troppe risorse
                sleep(Duration::from_millis(100)).await;
            }
        }

        // Aggiorna la finestra per mantenerla reattiva
        if let Some(ref mut win) = window {
            if win.is_open() {
                win.update();  // Continua ad aggiornare la finestra
            } else {
                break;
            }
        }
    }

    println!("Receiver fermato.");
    Ok(())
}
