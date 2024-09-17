use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use std::error::Error;
use scrap::{Capturer, Display};
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

//La funzione cattura continuamente il contenuto dello schermo
//Lo comprime e lo invia ai client connessi tramite un canale di broadcast
async fn capture_screen(sender: &broadcast::Sender<Vec<u8>>) -> Result<(), Box<dyn Error>> {
    let display = Display::primary()?;
    let mut capturer = Capturer::new(display)?; //Utilizzato per iniziare la cattura

    loop {
        let width = capturer.width();
        let height = capturer.height();
        match capturer.frame() {
            Ok(frame) => {
                println!("Frame catturato con successo, compressione in corso...");
                let jpeg_frame = compress_frame_to_jpeg(&frame, width, height).await?;
                let frame_size = (jpeg_frame.len() as u32).to_be_bytes();  // Converti la lunghezza del frame in 4 byte

                println!("Dimensione frame: {} byte", jpeg_frame.len());
                println!("Contenuto del frame (primi 10 byte): {:?}", &jpeg_frame[..10]);

                // Invia la dimensione del frame seguita dai dati del frame
                if let Err(e) = sender.send([&frame_size[..], &jpeg_frame[..]].concat()) {
                    eprintln!("Errore nell'invio del frame: {}", e);
                }
                println!("Frame compresso e inviato.");
                sleep(Duration::from_millis(100)).await;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                sleep(Duration::from_millis(100)).await;
                continue;
            }
            Err(e) => {
                eprintln!("Errore nella cattura del frame: {:?}", e);
                sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

pub async fn start_caster(addr: &str) -> Result<(), Box<dyn Error>> {
    //Listener TCP ascolta le connessioni in entrata sull'indirizzo specificato
    let listener = TcpListener::bind(addr).await?;
    //Canale broadcast: invio frame a tutti i client connessi
    let (tx, _rx) = broadcast::channel::<Vec<u8>>(10); // Canale per trasmettere frame a più receiver
    let tx = Arc::new(tx); // Avvolgi `tx` in un Arc per la condivisione sicura tra task

    let tx_clone = Arc::clone(&tx);
    tokio::spawn(async move {
        loop {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut rx = tx_clone.subscribe();  // Ogni receiver si abbona al canale

                tokio::spawn(async move {
                    while let Ok(frame) = rx.recv().await {
                        println!("Ricevuto frame dal canale.");
                        // Leggi i primi 4 byte per la dimensione del frame
                        if frame.len() < 4 {
                            eprintln!("Errore: frame troppo piccolo per contenere la dimensione.");
                            break;
                        }
                        let frame_size_bytes = &frame[0..4];
                        let frame_data = &frame[4..];
                        let frame_size = u32::from_be_bytes(frame_size_bytes.try_into().unwrap());

                        println!("Inviando frame di dimensione: {} byte", frame_size);

                        if socket.write_all(&frame_size_bytes).await.is_err() {
                            eprintln!("Errore nell'invio della dimensione del frame");
                            break;
                        }
                        if socket.write_all(&frame_data).await.is_err() {
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

    capture_screen(&*Arc::clone(&tx)).await?;

    Ok(())
}
