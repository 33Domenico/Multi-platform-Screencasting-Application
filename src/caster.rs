extern crate scrap;
use scrap::{Capturer, Display};
use std::io::ErrorKind;
use std::thread;
use std::time::Duration;
use std::net::TcpStream;
use std::io::{self, Write};
use std::io::Cursor;
use image::{ImageBuffer, RgbImage, DynamicImage, ImageOutputFormat};
use std::error::Error;

fn compress_frame_to_jpeg(frame: &[u8], width: usize, height: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    // Crea un buffer RGB per l'immagine
    let mut img_buffer: RgbImage = ImageBuffer::new(width as u32, height as u32);

    // Copia i pixel dal frame grezzo al buffer immagine (converte da BGRA a RGB)
    for (i, pixel) in img_buffer.pixels_mut().enumerate() {
        let idx = i * 4;
        *pixel = image::Rgb([frame[idx + 2], frame[idx + 1], frame[idx]]); // BGRA -> RGB
    }

    // Converti il buffer immagine in una DynamicImage per poterlo salvare come JPEG
    let img = DynamicImage::ImageRgb8(img_buffer);

    // Comprime l'immagine in formato JPEG
    let mut jpeg_data = Vec::new();
    img.write_to(&mut Cursor::new(&mut jpeg_data), ImageOutputFormat::Jpeg(80))?; // 80 è la qualità del JPEG

    Ok(jpeg_data)
}

pub fn send_frame(frame: &[u8], addr: &str) -> io::Result<()> {
    let mut stream = TcpStream::connect(addr)?;

    // Invia i dati del frame
    stream.write_all(frame)?;
    Ok(())
}

pub fn capture_screen() -> Result<(), Box<dyn std::error::Error>> {
    // Seleziona il display (primo monitor)
    let display = Display::primary()?;

    /** equivalente a

    let display = match Display::primary() {
    Ok(display) => display,
    Err(e) => return Err(e),
    };

    **/

    // Crea il catturatore
    let mut capturer = Capturer::new(display).expect("Failed to start screen capturing");

    let receiver_addr = "127.0.0.1:12345";  // Indirizzo del receiver

    loop {

        let width = capturer.width();
        let height = capturer.height();
        // Prova a catturare un frame
        match capturer.frame() {
            Ok(frame) => {

                println!("Captured a frame of size {}x{}", width, height);

                // Puoi ora processare i dati del frame, che sono una slice di pixel raw
                // Ogni pixel è rappresentato da 4 byte: BGRA (Blue, Green, Red, Alpha)
                // Qui puoi salvare, trasmettere o manipolare il frame

                // Comprime il frame in JPEG
                let jpeg_frame = compress_frame_to_jpeg(&frame, width, height)?;

                // Invia il frame compresso al receiver
                send_frame(&jpeg_frame, receiver_addr)?;

                // Attendere un po' prima della prossima cattura
                thread::sleep(Duration::from_millis(100));
            }
            Err(error) => {
                if error.kind() == ErrorKind::WouldBlock {
                    // Se non ci sono frame pronti, aspetta e riprova
                    thread::sleep(Duration::from_millis(100));
                    continue;
                } else {
                    return Err(Box::new(error));
                }
            }
        }
    }
}

