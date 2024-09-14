extern crate scrap;
use scrap::{Capturer, Display};
use std::io::ErrorKind;
use std::thread;
use std::time::Duration;

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

    loop {
        // Prova a catturare un frame
        match capturer.frame() {
            Ok(frame) => {
                println!("Captured a frame of size {}x{}", capturer.width(), capturer.height());

                // Puoi ora processare i dati del frame, che sono una slice di pixel raw
                // Ogni pixel è rappresentato da 4 byte: BGRA (Blue, Green, Red, Alpha)
                // Qui puoi salvare, trasmettere o manipolare il frame

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

