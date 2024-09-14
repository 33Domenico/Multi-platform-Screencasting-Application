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

async fn capture_screen(sender: broadcast::Sender<Vec<u8>>) -> Result<(), Box<dyn Error>> {
    let display = Display::primary()?;
    let mut capturer = Capturer::new(display)?;

    loop {
        let width = capturer.width();
        let height = capturer.height();
        match capturer.frame() {
            Ok(frame) => {
                let jpeg_frame = compress_frame_to_jpeg(&frame, width, height).await?;
                sender.send(jpeg_frame).unwrap();  // Invia il frame a tutti i receiver
                sleep(Duration::from_millis(100)).await;  // Intervallo tra i frame
            }
            Err(e) => {
                eprintln!("Errore nella cattura del frame: {:?}", e);
                sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

pub async fn start_caster(addr: &str) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(addr).await?;
    let (tx, _rx) = broadcast::channel(10); // Canale per trasmettere frame a più receiver

    // Avvia la cattura dello schermo in un task separato
    tokio::spawn(async move {
        if let Err(e) = capture_screen(tx).await {
            eprintln!("Errore durante la cattura dello schermo: {}", e);
        }
    });

    println!("Caster in ascolto su {}", addr);

    loop {
        let (mut socket, _) = listener.accept().await?;
        let mut rx = tx.subscribe();  // Ogni receiver si abbona al canale

        // Gestisci ogni connessione in modo asincrono
        tokio::spawn(async move {
            while let Ok(frame) = rx.recv().await {
                if socket.write_all(&frame).await.is_err() {
                    eprintln!("Errore nell'invio del frame");
                    break;
                }
            }
        });
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = "127.0.0.1:12345";
    start_caster(addr).await?;
    Ok(())
}


