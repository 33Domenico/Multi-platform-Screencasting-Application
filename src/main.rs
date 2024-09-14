use std::fmt::Error;
use crate::receiver::receive_frame;
use crate::caster::capture_screen;
mod caster;
mod receiver;
mod ui;
mod network;

fn main() -> Result<(), Box<dyn std::error::Error>> {
let receiver_addr = "127.0.0.1:12345";  // Indirizzo su cui il receiver ascolta
let output_file = "received_frame.jpeg";  // File dove salvare l'immagine ricevuta

capture_screen()?;


receive_frame(receiver_addr, output_file)?;

println!("Ricezione completata");
Ok(())
}
