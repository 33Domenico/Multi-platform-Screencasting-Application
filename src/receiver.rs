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
    pub(crate) fn reset_parameter(&mut self){
        self.recording = false;
        self.frame_count = 0;
        self.frame_width = None;
        self.frame_height = None;
        self.last_frame_time = None;
        self.paused_duration = Duration::new(0, 0);
        self.pause_start_time=None;
    }

    //crea la cartellla di output, e frames contenuta in putput e inizializza i parametri per la registrazione
    pub fn start_recording(&mut self) -> io::Result<()> {
        if self.recording {
            return Ok(());
        }
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        self.output_dir = format!("recording_{}", timestamp);
        fs::create_dir_all(&self.output_dir)?;//creazione cartella di output
        let frames_dir = Path::new(&self.output_dir).join("frames");
        fs::create_dir_all(&frames_dir)?;//creazione cartella per salvare i frame

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
        //Seil primo frame registrato ha frame_width , salva la dimesione  come riferimento futuro
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
        //generazione percorso per il slvataggio
        let frame_path = Path::new(&self.output_dir)
            .join("frames")
            .join(format!("frame_{:06}.png", self.frame_count));

        img.save(&frame_path)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        self.frame_count += 1;
        self.last_frame_time = Some(Instant::now());
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
        //turata totale video, eliminando il tempo di pausa
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

        let output_dir = self.output_dir.clone();
        let framerate = self.framerate;
        let frame_width = self.frame_width;
        let frame_height = self.frame_height;

        // Resetta lo stato immediatamente
        self.reset_parameter();

        tokio::spawn(async move {
            let conversion_result = convert_to_mp4(&output_dir, framerate, frame_width, frame_height);

            if conversion_result.is_ok() {
                let _ = delete_frames(&output_dir);
            }
        });

        Ok(())
    }

}
fn delete_frames(output_dir: &str) -> io::Result<()> {
    fs::remove_dir_all(Path::new(output_dir).join("frames"))?;
    Ok(())
}
fn convert_to_mp4(output_dir: &str, framerate: f64, width: Option<u32>, height: Option<u32>) -> io::Result<()> {
    if width.is_none() || height.is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Dimensioni frame non valide"
        ));
    }

    let output = Command::new("ffmpeg")
        .args(&[
            "-framerate", &format!("{:.2}", framerate),
            "-i", &format!("{}/frames/frame_%06d.png", output_dir),
            "-vf", "scale=ceil(iw/2)*2:ceil(ih/2)*2", // Scala il video per garantire dimensioni pari,ceil() arrotonda per eccesso
            "-c:v", "libx264",
            "-pix_fmt", "yuv420p",
            "-preset", "fast",  // Ottimizzazione per velocità
            "-crf", "23",//Qualità del video
            "-y",//Sovrascrive il file se esiste
            &format!("{}/output.mp4", output_dir)
        ])
        .output()?;

    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
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
    addr: &str, // Indirizzo ip caster
    stop_signal: Arc<AtomicBool>, // Segale per ferame il reciver
    shared_frame: Arc<RwLock<SharedFrame>>, // Frame condiviso con la UI(Per la visaualizzazione della stream)
    receiver_state: Arc<RwLock<ReceiverState>>,
    connected_to_caster: Arc<AtomicBool>//flag per verificare la connessione al caster
) -> io::Result<()> {
    // Impostazione del timeout utile per verificare la presenza di un caster, se il cster non risponde entro 2 secondi il reciver termina anziche rimanere bloccato
    let read_timeout = Duration::from_secs(2);
    let mut stream; //todo
    match timeout(read_timeout, TcpStream::connect(addr)).await { //TcpStream::connect(addr) è un operazione asincrona che richiede del tempo, se il future termina entro il tempo limite vine restituito il risultato altrimenti viene restituito un errore di timeout
        Ok(Ok(s)) => stream=s,//todo
        Ok(Err(e)) => {
            eprintln!("Errore di connessione al caster: {}", e);
            return Err(e);
        }
        Err(_) => {
            eprintln!("Timeout di connessione al caster scaduto.");
            //todo(problmea del frame duplicato)
            return Err(io::Error::new(io::ErrorKind::TimedOut, "Timeout di connessione al caster scaduto, controlla l'indirizzo IP inserito e riprova."));
        }
    };
    // Se si è connessi al caster si imposta il flag connected_to_caster a true
    connected_to_caster.store(true, Ordering::SeqCst);

    let read_timeout = Duration::from_secs(2);
    let mut no_frame_received = false; //flag per verificare se non sono stati ricevuti frame

    while !stop_signal.load(Ordering::SeqCst) {// Finche non si riceve un segnale di stop
        let mut size_buf = [0u8; 4];
        //Vine fornito un tempo di 2 sceondi per permettere allo stream.read_exact di leggere esattamente 4 byte dello strem(che vengono a loro volta rimossi da esso(cosi punta ai dati successivi ovvero al frame))
        match timeout(read_timeout, stream.read_exact(&mut size_buf)).await {
            Ok(Ok(_)) => {
                let frame_size = u32::from_be_bytes(size_buf) as usize; // converte i 4 byte in un intero senza segno di 32 bit
                println!("Ricevuto frame di dimensione: {} byte", frame_size);
                //Acquisizione del lock in scrittura per modificare lo stato del receiver
                if let Ok(mut state) = receiver_state.write() {
                    state.is_paused = false;
                    state.last_frame_received = Some(Instant::now());
                }
                // Se il frame è piu grande di 10_000_000 byte si restituisce un errore
                if frame_size > 10_000_000 {
                    eprintln!("Frame troppo grande: {} byte", frame_size);
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Frame troppo grande"));
                }

                let mut buffer = vec![0u8; frame_size]; // Allocazione di un buffer di dimensione frame_size(Per recuperare il frame)
                stream.read_exact(&mut buffer).await?;//lettura del frame dallo stream, e immisione nel buffer
                //Decodifica dell immagine
                let img = ImageReader::new(std::io::Cursor::new(buffer))
                    .with_guessed_format()//identifica automaticamente il formato
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Errore nel formato dell'immagine: {}", e)))? //errore nel formato dell'immagine, Coversione errore in errore di io
                    .decode()//trasforma i dati in un immagine
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Errore durante la decodifica dell'immagine: {}", e)))?; //errore nella decodifica dell'immagine, Coversione errore in errore di io

                let img = img.to_rgba8();//Conversione dell'immagine in RGBA8
                let (width, height) = img.dimensions();
                //Scritture dei dati nel frame condiviso
                if let Ok(mut shared) = shared_frame.write() {
                    shared.buffer = img.to_vec();
                    shared.width = width as usize;
                    shared.height = height as usize;
                    shared.new_frame = true;
                }
                //Salvataggio del frame se il receiver è in modalità di registrazione
                if let Ok(mut receiver_state)=receiver_state.write(){
                    receiver_state.save_frame(&img)?;
                }
            }

            Ok(Err(e)) => {
                eprintln!("Errore durante la lettura della dimensione del frame: {}", e);

                // Chiudi la connessione solo se è un errore irreversibile(La connessione con il caster si è chiusa)
                if e.kind() == io::ErrorKind::UnexpectedEof || e.raw_os_error() == Some(10054)  {
                    if let Ok(mut receiver_state) = receiver_state.write() {
                        if receiver_state.recording {
                            let _ = receiver_state.stop_recording();
                        }
                    }
                    connected_to_caster.store(false, Ordering::SeqCst);//todo
                    return Err(io::Error::new(e.kind(), "Connessione con il caster interrotta"));
                }

                // Riprova la connessione invece di terminare
                sleep(Duration::from_secs(1)).await;
                continue;
            }

            Err(_) => {
                println!("Timeout scaduto, nessun frame ricevuto.");
                // Se non sono stati ricevuti frame per 2 secondi, metti in pausa lo stream
                if let Ok(mut state) = receiver_state.write() {
                    if !state.is_paused {
                        state.is_paused = true;
                        println!("Stream in pausa");
                    }
                }
                // Se il flag no_frame_received è falso, si imposta il tempo di inizio della pausa,e il flag a true(Utile in caso di regitrazione, per individuare il corretto numero di fps da attribuire al video)
                if !no_frame_received {
                    if let Ok(mut receiver_state) = receiver_state.write() {
                        if receiver_state.pause_start_time.is_none() {
                            receiver_state.pause_start_time = Some(Instant::now()- Duration::from_millis(1975));//todo
                        }
                    }
                    no_frame_received = true;
                }
                sleep(Duration::from_millis(100)).await; //dormi per 100 millisecondi, prima di riprovare
            }
        }
        //accumulo il tempo di pausa all interno del campo duration del receiver_state
        if no_frame_received {
            if let Ok(mut receiver_state) = receiver_state.write() {
                if let Some(pause_start_time) = receiver_state.pause_start_time {
             receiver_state.paused_duration += pause_start_time.elapsed(); //Tempo passato dalla pausa
                receiver_state.pause_start_time = None; //resettare il tempo di inizio della pausa,per pemettermi(Necessario per logica di accumulo del tempo di pausa)
                }
                println!("{:?}",receiver_state.paused_duration);
                no_frame_received = false;//Necessario per logica di accumulo del tempo di pausa

            }
        }

    }

    if let Ok(mut receiver_state) = receiver_state.write() {
        if receiver_state.recording {
            receiver_state.stop_recording()?; //termina registrazzione se avviata nel caso in cui si interrompa lo stream
        }
    }

    if let Ok(mut shared) = shared_frame.write() {
        shared.buffer.clear();
        shared.new_frame = false;
    }
    connected_to_caster.store(false, Ordering::SeqCst);

    println!("Receiver fermato.");
    Ok(())
}