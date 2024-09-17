use tokio::net::TcpStream;
use std::io::{self};
use tokio::io::AsyncReadExt;
use image::ImageReader;
use minifb::{Window, WindowOptions};

pub async fn receive_frame(addr: &str) -> io::Result<()> {
    // Connessione al caster
    let mut stream = TcpStream::connect(addr).await?;

    // Variabili per gestire la finestra e la risoluzione
    let mut window: Option<Window> = None;
    let mut width: usize = 0;
    let mut height: usize = 0;

    loop {
        let mut size_buf = [0u8; 4];  // Buffer per la dimensione del frame

        // Leggi i primi 4 byte per ottenere la dimensione del frame
        match stream.read_exact(&mut size_buf).await {
            Ok(_) => {
                let frame_size = u32::from_be_bytes(size_buf) as usize;
                println!("Ricevuto frame di dimensione: {} byte", frame_size);

                if frame_size > 10_000_000 {
                    eprintln!("Frame troppo grande: {} byte", frame_size);
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Frame troppo grande"));
                }

                // Leggi il frame JPEG
                let mut buffer = vec![0u8; frame_size];
                stream.read_exact(&mut buffer).await?;

                // Decodifica il frame JPEG e gestisci eventuali errori
                let img = ImageReader::new(std::io::Cursor::new(buffer))
                    .with_guessed_format()
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Errore nel formato dell'immagine: {}", e)))?
                    .decode()
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Errore durante la decodifica dell'immagine: {}", e)))?;

                println!("Frame decodificato con successo!");

                // Ottieni le dimensioni dell'immagine
                let img = img.to_rgba8();
                let (w, h) = img.dimensions();
                println!("Dimensioni del frame: {}x{}", w, h);

                // Se la finestra non esiste, creala
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

                // Se esiste la finestra, visualizza i frame
                if let Some(ref mut win) = window {
                    // Converti l'immagine in un buffer di pixel (u32 RGBA)
                    let buffer: Vec<u32> = img
                        .pixels()
                        .map(|p| {
                            let rgba = p.0;
                            let r = rgba[0] as u32;
                            let g = rgba[1] as u32;
                            let b = rgba[2] as u32;
                            let a = rgba[3] as u32;
                            (r << 16) | (g << 8) | b | (a << 24)  // Assicurati che l'ordine dei colori sia corretto
                        })
                        .collect();

                    // Mostra il buffer nella finestra
                    if win.is_open() {
                        println!("Visualizzando il frame...");
                        win.update_with_buffer(&buffer, width, height)
                            .unwrap();
                    } else {
                        eprintln!("La finestra è stata chiusa.");
                        break;
                    }
                }
            }
            Err(e) => {
                eprintln!("Errore durante la lettura della dimensione del frame: {}", e);
                return Err(e);
            }
        }
    }
}


