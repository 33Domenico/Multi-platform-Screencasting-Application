use tokio::net::TcpStream;
use std::io;
use tokio::io::AsyncReadExt;
use image::ImageReader;
use minifb::{Window, WindowOptions};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}, Mutex};
use tokio::time::{sleep, Duration, timeout};

// Struttura per condividere il frame tra i thread
pub struct SharedFrame {
    pub buffer: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub new_frame: bool,
}

impl Default for SharedFrame {
    fn default() -> Self {
        Self {
            buffer: Vec::new(),
            width: 0,
            height: 0,
            new_frame: false,
        }
    }
}

pub async fn receive_frame(
    addr: &str,
    stop_signal: Arc<AtomicBool>,
    shared_frame: Arc<Mutex<SharedFrame>>,
) -> io::Result<()> {
    let mut stream = TcpStream::connect(addr).await?;
    let read_timeout = Duration::from_secs(2);

    while !stop_signal.load(Ordering::SeqCst) {
        let mut size_buf = [0u8; 4];

        match timeout(read_timeout, stream.read_exact(&mut size_buf)).await {
            Ok(Ok(_)) => {
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
                let (width, height) = img.dimensions();

                // Aggiorna il frame condiviso
                if let Ok(mut shared) = shared_frame.lock() {
                    shared.buffer = img.into_raw();
                    shared.width = width as usize;
                    shared.height = height as usize;
                    shared.new_frame = true;
                }
            }
            Ok(Err(e)) => {
                eprintln!("Errore durante la lettura della dimensione del frame: {}", e);

                let frame_size = u32::from_be_bytes(size_buf) as usize;
                if frame_size == 0 {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Il caster ha chiuso la trasmissione."));
                }

                return Err(e);
            }
            Err(_) => {
                println!("Timeout scaduto, nessun frame ricevuto.");
                sleep(Duration::from_millis(100)).await;
            }
        }
    }

    println!("Receiver fermato.");
    Ok(())
}