use tokio::net::TcpStream;
use tokio::io::AsyncReadExt;
use image::ImageReader;
use std::sync::{Arc, atomic::{AtomicBool, Ordering},RwLock};
use tokio::time::{sleep, Duration, timeout};
use std::fs;
use std::path::Path;
use chrono::Local;
use std::io::{self};
use image::RgbaImage;
use std::process::Command;
use std::time::{ Instant};

pub struct ReceiverState {
    pub recording: bool,
    pub(crate) frame_count: u32,
    pub(crate) output_dir: String,
    current_video_path: Option<String>,
    frame_width: Option<u32>,
    frame_height: Option<u32>,
    last_frame_time: Option<Instant>,
    start_time: Option<Instant>,
    paused_duration: Duration,
    pause_start_time: Option<Instant>,
    pub framerate: f64,
    pub is_paused: bool,
    last_frame_received: Option<Instant>,
}

impl ReceiverState {
    pub fn new() -> Self {
        Self {
            recording: false,
            frame_count: 0,
            output_dir: String::new(),
            current_video_path: None,
            frame_width: None,
            frame_height: None,
            last_frame_time: None,
            start_time: None,
            paused_duration: Duration::new(0, 0),
            pause_start_time: None,
            framerate: 30.0,
            is_paused: false,
            last_frame_received: None,
        }
    }
    fn reset_parameter(&mut self){
        self.recording = false;
        self.frame_count = 0;
        self.frame_width = None;
        self.frame_height = None;
        self.last_frame_time = None;
        self.paused_duration = Duration::new(0, 0);
        self.pause_start_time=None;
    }

    pub fn start_recording(&mut self) -> io::Result<()> {
        if self.recording {
            return Ok(());
        }
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        self.output_dir = format!("recording_{}", timestamp);
        fs::create_dir_all(&self.output_dir)?;
        let frames_dir = Path::new(&self.output_dir).join("frames");
        fs::create_dir_all(&frames_dir)?;

        self.recording = true;
        self.frame_count = 0;
        self.start_time = Some(Instant::now());
        self.last_frame_time = Some(Instant::now());
        self.paused_duration = Duration::new(0, 0);
        println!("Started recording in: {}", self.output_dir);
        Ok(())
    }

    pub fn save_frame(&mut self, img: &RgbaImage) -> io::Result<()> {
        if !self.recording {
            return Ok(());
        }

        let (width, height) = img.dimensions();
        if width < 10 || height < 10 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Frame dimensions too small (minimum 10x10 pixels required)",
            ));
        }

        if self.frame_width.is_none() {
            self.frame_width = Some(width);
            self.frame_height = Some(height);
        }

        if Some(width) != self.frame_width || Some(height) != self.frame_height {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Frame dimensions changed during recording",
            ));
        }

        let frame_path = Path::new(&self.output_dir)
            .join("frames")
            .join(format!("frame_{:06}.png", self.frame_count));

        img.save(&frame_path)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        self.frame_count += 1;
        self.last_frame_time = Some(Instant::now());
        Ok(())
    }

    fn delete_frames(&self) -> io::Result<()> {
        let frames_dir = Path::new(&self.output_dir).join("frames");
        if frames_dir.exists() {
            fs::remove_dir_all(frames_dir)?;
            println!("Frames deleted successfully.");
        }
        Ok(())
    }
    pub fn stop_recording(&mut self) -> io::Result<()> {
        if !self.recording {
            return Ok(());
        }
        println!("Stopping recording. Frames saved: {}", self.frame_count);

        if self.frame_count == 0 {
            self.reset_parameter();
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "No frames were recorded",
            ));
        }

        let duration = self.last_frame_time.unwrap().duration_since(self.start_time.unwrap())- self.paused_duration;
        self.framerate = self.frame_count as f64 / duration.as_secs_f64();
        println!("Framerate effettivo: {:.2} fps", self.framerate );

        let metadata = format!(
            "frames: {}\nfps: {:.2}\nwidth: {}\nheight: {}\nstart_time: {}\n",
            self.frame_count,
            self.framerate,
            self.frame_width.unwrap_or(0),
            self.frame_height.unwrap_or(0),
            Local::now().to_rfc3339()
        );

        fs::write(Path::new(&self.output_dir).join("metadata.txt"), metadata)?;
        match self.convert_to_mp4() {
            Ok(_) => {
                self.delete_frames()?;
            }
            Err(e) => {
                self.reset_parameter();
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    e.to_string(),
                ));
            }
        }
        self.reset_parameter();
        Ok(())
    }

    fn convert_to_mp4(&mut self) -> io::Result<()> {
        if !Command::new("ffmpeg").arg("-version").output().is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "ffmpeg not found. Please install ffmpeg to enable video conversion.",
            ));
        }

        let output_path = Path::new(&self.output_dir)
            .join("output.mp4")
            .to_string_lossy()
            .to_string();

        let frames_pattern = Path::new(&self.output_dir)
            .join("frames")
            .join("frame_%06d.png")
            .to_string_lossy()
            .to_string();

        let output = Command::new("ffmpeg")
            .args([
                "-framerate", &format!("{:.2}", self.framerate ),
                "-i", &frames_pattern,
                "-vf", "scale=ceil(iw/2)*2:ceil(ih/2)*2",
                "-c:v", "libx264",
                "-pix_fmt", "yuv420p",
                "-preset", "medium",
                "-crf", "23",
                "-r", &format!("{:.2}", self.framerate ),
                "-y",
                &output_path
            ])
            .output()?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to convert video: {}", error),
            ));
        }

        self.current_video_path = Some(output_path.clone());
        println!("Video successfully created at: {}", output_path);

        Ok(())
    }
}
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
    shared_frame: Arc<RwLock<SharedFrame>>,
    receiver_state: Arc<RwLock<ReceiverState>>,
    connected_to_caster: Arc<AtomicBool>
) -> io::Result<()> {

    let read_timeout = Duration::from_secs(2);
    match timeout(read_timeout, TcpStream::connect(addr)).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            eprintln!("Errore di connessione al caster: {}", e);
            return Err(e);
        }
        Err(_) => {
            eprintln!("Timeout di connessione al caster scaduto.");
            return Err(io::Error::new(io::ErrorKind::TimedOut, "Timeout di connessione al caster scaduto, controlla l'indirizzo IP inserito e riprova."));
        }
    };

    connected_to_caster.store(true, Ordering::SeqCst);

    let mut stream = TcpStream::connect(addr).await?;
    let read_timeout = Duration::from_secs(2);
    let mut no_frame_received = false;

    while !stop_signal.load(Ordering::SeqCst) {
        let mut size_buf = [0u8; 4];

        match timeout(read_timeout, stream.read_exact(&mut size_buf)).await {
            Ok(Ok(_)) => {
                let frame_size = u32::from_be_bytes(size_buf) as usize;
                println!("Ricevuto frame di dimensione: {} byte", frame_size);

                if let Ok(mut state) = receiver_state.write() {
                    state.is_paused = false;
                    state.last_frame_received = Some(Instant::now());
                }

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

                if let Ok(mut shared) = shared_frame.write() {
                    shared.buffer = img.to_vec();
                    shared.width = width as usize;
                    shared.height = height as usize;
                    shared.new_frame = true;
                }

                if let Ok(mut receiver_state)=receiver_state.write(){
                    receiver_state.save_frame(&img)?;
                }
            }

            Ok(Err(e)) => {
                eprintln!("Errore durante la lettura della dimensione del frame: {}", e);

                let frame_size = u32::from_be_bytes(size_buf) as usize;
                if frame_size == 0 {
                    if let Ok(mut receiver_state) = receiver_state.write() {
                        if receiver_state.recording {
                            receiver_state.stop_recording()?;
                        }
                    }
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Il caster ha chiuso la trasmissione."));
                }

                return Err(e);
            }

            Err(_) => {
                println!("Timeout scaduto, nessun frame ricevuto.");
                if let Ok(mut state) = receiver_state.write() {
                    if !state.is_paused {
                        state.is_paused = true;
                        println!("Stream in pausa");
                    }
                }

                if !no_frame_received {
                    if let Ok(mut receiver_state) = receiver_state.write() {
                        if receiver_state.pause_start_time.is_none() {
                            receiver_state.pause_start_time = Some(Instant::now());
                        }
                    }
                    no_frame_received = true;
                }
                sleep(Duration::from_millis(100)).await;
            }
        }
        if no_frame_received {
            if let Ok(mut receiver_state) = receiver_state.write() {
                if let Some(pause_start_time) = receiver_state.pause_start_time {
             receiver_state.paused_duration += pause_start_time.elapsed();
            receiver_state.pause_start_time = None;
                }
                println!("{:?}",receiver_state.paused_duration);
                no_frame_received = false;

            }
        }

    }

    if let Ok(mut receiver_state) = receiver_state.write() {
        if receiver_state.recording {
            receiver_state.stop_recording()?;
        }
    }

    println!("Receiver fermato.");
    Ok(())
}