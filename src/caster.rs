use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use std::error::Error;
use scrap::{Capturer, Display};
use image::{ImageBuffer, RgbImage, DynamicImage, ImageOutputFormat};
use std::io::Cursor;
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
    img.write_to(&mut Cursor::new(&mut jpeg_data), ImageOutputFormat::Jpeg(80))?;
    Ok(jpeg_data)
}

async fn capture_screen(sender: &broadcast::Sender<Vec<u8>>) -> Result<(), Box<dyn Error>> {
    let display = Display::primary()?;
    let mut capturer = Capturer::new(display)?;

    loop {
        let width = capturer.width();
        let height = capturer.height();
        match capturer.frame() {
            Ok(frame) => {
                let jpeg_frame = compress_frame_to_jpeg(&frame, width, height).await?;
                if let Err(e) = sender.send(jpeg_frame) {
                    eprintln!("Errore nell'invio del frame: {}", e);
                }
                // Attendi prima di catturare il prossimo frame
                sleep(Duration::from_millis(100)).await;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Non è ancora disponibile un frame, aspetta un po' e riprova
                sleep(Duration::from_millis(100)).await;
                continue;
            }
            Err(e) => {
                // Gestione di altri tipi di errori
                eprintln!("Errore nella cattura del frame: {:?}", e);
                sleep(Duration::from_millis(100)).await;
            }
        }
    }
}


pub async fn start_caster(addr: &str) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(addr).await?;
    let (tx, _rx) = broadcast::channel::<Vec<u8>>(10); // Canale per trasmettere frame a più receiver
    let tx = Arc::new(tx); // Avvolgi `tx` in un Arc per la condivisione sicura tra task

    let tx_clone = Arc::clone(&tx);
    tokio::spawn(async move {
        loop {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut rx = tx_clone.subscribe();  // Ogni receiver si abbona al canale

                tokio::spawn(async move {
                    while let Ok(frame) = rx.recv().await {
                        if socket.write_all(&frame).await.is_err() {
                            eprintln!("Errore nell'invio del frame");
                            break;
                        }
                    }
                });
            } else {
                eprintln!("Errore nell'accettare la connessione");
            }
        }
    });

    // Passa un clone di `tx` alla funzione di cattura dello schermo
    capture_screen(&*Arc::clone(&tx)).await?;

    Ok(())
}

