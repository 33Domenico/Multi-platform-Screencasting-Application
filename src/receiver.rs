use std::fs;
use std::path::Path;
use tokio::net::TcpStream;
use std::fs::File;
use std::io::{self, Write};
use tokio::io::AsyncReadExt;
use std::time::{SystemTime, UNIX_EPOCH};

//Si connette a un server TCP (caster) e riceve frame che sono salvati come JPEG
pub async fn receive_frame(addr: &str, output_folder: &str) -> io::Result<()> {
    //Creazione connessione asincrona al server
    let mut stream = TcpStream::connect(addr).await?;

    //Verifica se la cartella esiste, altrimenti creala
    let path = Path::new(output_folder);
    if !path.exists() {
        //Se la cartella specificata per salvare i frame non esiste viene creata
        match fs::create_dir_all(output_folder) {
            Ok(_) => {
                println!("Cartella {} creata con successo.", output_folder);
            },
            Err(e) => {
                eprintln!("Errore durante la creazione della cartella {}: {}", output_folder, e);
                return Err(e);
            }
        }
    }

    loop {
        let mut size_buf = [0u8; 4];  //Buffer per leggere la dimensione del frame

        //Leggi i primi 4 byte che contengono la dimensione del frame
        match stream.read_exact(&mut size_buf).await {
            Ok(_) => {
                let frame_size = u32::from_be_bytes(size_buf) as usize;
                println!("Ricevuto frame di dimensione: {} byte", frame_size);

                if frame_size > 10_000_000 {
                    eprintln!("Dimensione del frame non valida: {} byte", frame_size);
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Frame troppo grande"));
                }

                let mut buffer = vec![0u8; frame_size];

                stream.read_exact(&mut buffer).await?;

                // Genera un nome unico per il file basato sul timestamp corrente
                let start = SystemTime::now();
                let since_the_epoch = start.duration_since(UNIX_EPOCH).expect("Time went backwards");
                let filename = format!("{}/received_frame_{}.jpeg", output_folder, since_the_epoch.as_secs());

                // Log per verificare il percorso completo del file
                println!("Salvando il file in: {}", filename);

                // Salva il frame come file JPEG
                let mut file = match File::create(&filename) {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("Errore durante la creazione del file {}: {}", filename, e);
                        return Err(e);
                    }
                };

                file.write_all(&buffer)?;

                println!("Frame ricevuto e salvato in {}", filename);
            }
            Err(e) => {
                eprintln!("Errore durante la lettura della dimensione del frame: {}", e);
                return Err(e);
            }
        }
    }
}
