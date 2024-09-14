use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write, Result};
use std::fs::File;

fn handle_client(mut stream: TcpStream, output_file: &str) -> Result<()> {
    let mut buffer = Vec::new();

    // Leggi i dati dal socket (riceve tutto il JPEG)
    stream.read_to_end(&mut buffer)?;

    // Scrivi il buffer in un file JPEG
    let mut file = File::create(output_file)?;
    file.write_all(&buffer)?;

    println!("Frame ricevuto e salvato in {}", output_file);
    Ok(())
}

pub fn receive_frame(addr: &str, output_file: &str) -> Result<()> {
    let listener = TcpListener::bind(addr)?;

    println!("In attesa di connessioni su {}", addr);

    // Accetta una singola connessione
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("Connessione accettata: {:?}", stream.peer_addr());
                handle_client(stream, output_file)?;
            }
            Err(e) => {
                eprintln!("Errore di connessione: {}", e);
            }
        }
    }

    Ok(())
}