use tokio::net::TcpStream;
use tokio::io::AsyncReadExt;
use image::ImageReader;
use minifb::{Window, WindowOptions};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}, Mutex};
use tokio::time::{sleep, Duration, timeout};
use tokio::task;
use std::fs;
use std::path::Path;
use chrono::Local;
use std::io::{self, Write};

pub struct ReceiverState {
    pub recording: bool,
    frame_count: u32,
    output_dir: String,
}

impl ReceiverState {
    pub fn new() -> Self {
        Self {
            recording: false,
            frame_count: 0,
            output_dir: String::new(),
        }
    }

    pub fn start_recording(&mut self) -> io::Result<()> {
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        self.output_dir = format!("recording_{}", timestamp);
        fs::create_dir_all(&self.output_dir)?;
        self.recording = true;
        self.frame_count = 0;
        println!("Iniziata registrazione in: {}", self.output_dir);
        Ok(())
    }

    pub fn save_frame(&mut self, img: &image::RgbaImage) -> io::Result<()> {
        if self.recording {
            let frame_path = Path::new(&self.output_dir)
                .join(format!("frame_{:06}.png", self.frame_count));
            img.save(&frame_path)
                .map_err(|e| io::Error::new(std::io::ErrorKind::Other, e))?;
            self.frame_count += 1;
        }
        Ok(())
    }

    pub fn stop_recording(&mut self) -> io::Result<()> {
        if self.recording {
            println!("Registrazione fermata. Frames salvati: {}", self.frame_count);
            let metadata = format!(
                "frames: {}\nfps: 30\nstart_time: {}\n",
                self.frame_count,
                Local::now().to_rfc3339()
            );
            fs::write(
                Path::new(&self.output_dir).join("metadata.txt"),
                metadata
            )?;

            // Converti in video se ffmpeg è disponibile
            if let Ok(()) = self.convert_to_video() {
                println!("Video creato con successo!");
            } else {
                println!("Non è stato possibile creare il video. I frame sono stati salvati in: {}", self.output_dir);
            }

            self.recording = false;
            self.frame_count = 0;
        }
        Ok(())
    }

    fn convert_to_video(&self) -> io::Result<()> {
        use std::process::Command;

        let status = Command::new("ffmpeg")
            .args(&[
                "-framerate", "30",
                "-i", &format!("{}/frame_%06d.png", self.output_dir),
                "-c:v", "libx264",
                "-pix_fmt", "yuv420p",
                &format!("{}/output.mp4", self.output_dir)
            ])
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "Errore durante la conversione del video"
            ))
        }
    }

}
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
    receiver_state: Arc<Mutex<ReceiverState>>
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
                    shared.buffer = img.to_vec();
                    shared.width = width as usize;
                    shared.height = height as usize;
                    shared.new_frame = true;
                }

                // Salva il frame se la registrazione è attiva
                if let Ok(mut receiver_state)=receiver_state.lock(){
                    receiver_state.save_frame(&img)?;
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