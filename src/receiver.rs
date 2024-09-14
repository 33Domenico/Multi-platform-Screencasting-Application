use tokio::net::TcpStream;
use std::fs::File;
use std::io::{self, Write};
use tokio::io::AsyncReadExt;

pub async fn receive_frame(addr: &str, output_file: &str) -> io::Result<()> {
    let mut stream = TcpStream::connect(addr).await?;
    let mut buffer = Vec::new();

    // Ricevi il frame
    stream.read_to_end(&mut buffer).await?;

    // Salva il frame come file JPEG
    let mut file = File::create(output_file)?;
    file.write_all(&buffer)?;

    println!("Frame ricevuto e salvato in {}", output_file);
    Ok(())
}


