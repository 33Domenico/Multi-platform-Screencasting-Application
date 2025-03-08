use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use eframe::epaint::Rect;
use scrap::{Capturer, Display};
use image::{ImageBuffer, RgbImage, DynamicImage};
use tokio::io::AsyncWriteExt;
use tokio::time::{sleep, Duration};
use device_query::{DeviceQuery, DeviceState, Keycode};


struct HotkeyState {
    paused: Arc<AtomicBool>,
    screen_blanked: Arc<AtomicBool>,
    terminate: Arc<AtomicBool>,
}


fn handle_hotkeys(hotkey_state: Arc<HotkeyState>) {
    let device_state = DeviceState::new();
    let mut last_keys = Vec::new();

    loop {
        let keys: Vec<Keycode> = device_state.get_keys();//prende l eleenco dei tasti premuti
        //se i tasti premuti sono diversi da quelli premuti nell iterazione precedente
        if keys != last_keys {

            if keys.contains(&Keycode::F1) {
                hotkey_state.paused.fetch_xor(true, Ordering::SeqCst);
                println!("Trasmissione {}.", if hotkey_state.paused.load(Ordering::SeqCst) { "paused" } else { "resumed" });
            }
            if keys.contains(&Keycode::F2) {
                hotkey_state.screen_blanked.fetch_xor(true, Ordering::SeqCst);
                println!("Schermo {}.", if hotkey_state.screen_blanked.load(Ordering::SeqCst) { "blanked" } else { "unblanked" });
            }
            if keys.contains(&Keycode::Escape) {
                hotkey_state.terminate.store(true, Ordering::SeqCst);
                println!("Terminazione richiesta.");
                break;
            }
        }

        last_keys = keys;
        std::thread::sleep(Duration::from_millis(100));
    }
}

async fn compress_frame_to_jpeg(frame: &[u8], width: usize, height: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut img_buffer: RgbImage = ImageBuffer::new(width as u32, height as u32);
    for (i, pixel) in img_buffer.pixels_mut().enumerate() {
        let idx = i * 4;
        *pixel = image::Rgb([frame[idx + 2], frame[idx + 1], frame[idx]]);//scambiamo i byte per avere il formato corretto da bgrx a rgb
    }
    let img = DynamicImage::ImageRgb8(img_buffer); // dinmic image serve per convertire l immagine in jpeg
    let mut jpeg_data = Vec::new();
    {
        let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut jpeg_data);
        encoder.encode_image(&img)?; //codifica l immagine in jpeg
    }
    Ok(jpeg_data)
}

async fn capture_screen(
    sender: &broadcast::Sender<Vec<u8>>, // Canale per inviare i frame
    stop_signal: Arc<AtomicBool>, // Segnale per fermare la cattura
    selected_area: Option<Rect>,// Area selezionata dello schermo se presente
    hotkey_state: Arc<HotkeyState>,// Stato dei tasti di controllo
    display_index: usize // Indice del display da catturare
) -> Result<(), Box<dyn Error>> {
    let displays = Display::all()?;
    if display_index >= displays.len() {
        return Err("Indice del display non valido".into());
    }
    let display = displays.into_iter().nth(display_index).unwrap();
    let mut capturer = Capturer::new(display)?; //crea un capturer raltivo al display selzionato per catturare u frame
    let mut last_frame: Option<Vec<u8>> = None; //utile per salvare l l'ultimo frame selezionato, usato per hartbeat e blank screen Un heartbeat è un segnale inviato periodicamente per indicare che una connessione è ancora attiva, anche se non ci sono nuovi dati da trasmettere.Nel tuo caso, quando il capturer.frame() non ha nuovi frame disponibili (errore WouldBlock), il codice invia l’ultimo frame catturato per mantenere la connessione attiva.
    while !stop_signal.load(Ordering::SeqCst) && !hotkey_state.terminate.load(Ordering::SeqCst) {
        //se l utente ha messo in pausa non catturare niente salta il ciclo
        if hotkey_state.paused.load(Ordering::SeqCst) {
            sleep(Duration::from_millis(100)).await;
            continue;
        }
        let width = capturer.width();
        let height = capturer.height();
        let default_width = width;
        let default_height = height;
        match capturer.frame() {
            Ok(frame) => {
                println!("Frame catturato con successo, compressione in corso...");
                let (selected_frame, cropped_width, cropped_height) = if let Some(area) = selected_area {
                    //identifico rettangolo da tagliare
                    let start_x = area.min.x as usize; //spigolo sinistro
                    let start_y = area.min.y as usize;
                    let end_x = area.max.x as usize; //spigolo destro
                    let end_y = area.max.y as usize;
                    //buffere riempito solo dei dati della regione selezionata
                    let mut cropped_frame = Vec::new();
                    //l immagine è salvata in formato rgba quidi 4 byte per pixel,quindi start indiex è y * width * 4 + start_x * 4
                    // vai a selziozionare i byte di ogni riga, dell area selzionata, per esempio area selezionata (100,100)a (300,300), nella prima iterazione selzioni i byte da y=100,per tutta la x=300(partendo da indice di inizo e fine), nella seconda iterazione selezioni i byte da y=101,per tutta la x=300(partendo da indice di inizo e fine) e cosi via
                    for y in start_y..end_y {
                        let start_index = y * width * 4 + start_x * 4;// y è la riga, dato che la riga ha un numero di valori pari alla larghezza per 4, io parto dall indice con quel valore di y (quellla riga) a cui aggiungo la x(colanna di quella riga)
                        let end_index = y * width * 4 + end_x * 4;
                        cropped_frame.extend_from_slice(&frame[start_index..end_index]); //estraggo i byte relativi alla regione selezionata
                    }
                    (cropped_frame, end_x - start_x, end_y - start_y)
                } else {
                    // se non sta area selezionata, ritorna il frame intero
                    (frame.to_vec(), width, height)
                };
                let jpeg_frame = if hotkey_state.screen_blanked.load(Ordering::SeqCst) {
                    let blank_frame = vec![0; cropped_width * cropped_height * 4];//crea un immagine nera ponendo i byte tutti a zero
                    compress_frame_to_jpeg(&blank_frame, cropped_width, cropped_height).await?//todo serve che sia asincrona?ogni volta che viene chiamata una funzione asincrona si cra un task??
                } else {
                    compress_frame_to_jpeg(&selected_frame, cropped_width, cropped_height).await?//comprime il frame in jpeg
                };
                last_frame = Some(jpeg_frame.clone()); //frame per heartbeat
                let frame_size = (jpeg_frame.len() as u32).to_be_bytes(); //Converti in byte la lunghezza del frame
                if let Err(e) = sender.send([&frame_size[..], &jpeg_frame[..]].concat()) {// invia frma al reciver, prima invii la lunghezza e poi il frame effettivo
                    eprintln!("Errore nell'invio del frame: {}", e);
                }
            },
            //l errore would block indica che non ci sono nuovi frame disponibili, quindi invio l ultimo frame catturato per mantenere la connessione attiva
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if hotkey_state.screen_blanked.load(Ordering::SeqCst) {
                    let jpeg_frame = if let Some(ref frame) = last_frame {
                        frame.clone()
                    } else {
                        let blank_frame = vec![0; default_width * default_height * 4];
                        compress_frame_to_jpeg(&blank_frame, default_width, default_height).await?
                    };
                    let frame_size = (jpeg_frame.len() as u32).to_be_bytes();
                    if let Err(e) = sender.send([&frame_size[..], &jpeg_frame[..]].concat()) {
                        eprintln!("Errore nell'invio del frame di blank screen: {}", e);
                    }
                } else if let Some(ref frame) = last_frame {//Heartbeat, inivio l ultimo frame ricevuto
                    let frame_size = (frame.len() as u32).to_be_bytes();
                    if let Err(e) = sender.send([&frame_size[..], &frame[..]].concat()) {
                        eprintln!("Errore nell'invio del frame di heartbeat: {}", e);
                    }
                }
                sleep(Duration::from_millis(10)).await;
            },
            Err(e) => {
                eprintln!("Errore nella cattura del frame: {:?}", e);
            }
        }
        sleep(Duration::from_millis(10)).await;
    }
    println!("Cattura dello schermo interrotta.");
    Ok(())
}



pub async fn start_caster(addr: &str, stop_signal: Arc<AtomicBool>, selected_area: Option<Rect>,display_index: usize,paused: Arc<AtomicBool>, screen_blanked: Arc<AtomicBool>,terminate: Arc<AtomicBool>) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(addr).await?; // avvio server TCP su addr che accetta connessioni dai reciver per inviare i frame
    let (tx, _rx) = broadcast::channel::<Vec<u8>>(100); // creazione canale di comunicazione per inviare i frame ai reciver, con buffer 100(è un canale asincorno, quindi non bloccante(bloccante per il task che invia il frame e quindi li riceve da capture_screen))
    let tx = Arc::new(tx);// condivido il canale tra i task, in modo che possano inviare i frame ai reciver(esiste un solo sender e più receiver)
    println!("Caster avviato su {}", addr);
    //inizializzo hotkey_state con i valori passati come argomento(che mi permettono di avere un collegamento tra ui e trasmissione)
    let hotkey_state = Arc::new(HotkeyState {
        paused,
        screen_blanked,
        terminate,
    });
    //creazione di un task che gestisce i comandi da tastiera, non si puo usare tokio::spawn perchè handle_hotkeys non è asincrono(scelta nostra)
    let hotkey_state_clone = Arc::clone(&hotkey_state);
    std::thread::spawn(move || {
        handle_hotkeys(hotkey_state_clone);
    });

    let tx_clone = Arc::clone(&tx); //clono il canale per passarlo al task che cattura i frame(che fara da sender)
    let stop_signal_clone = Arc::clone(&stop_signal); //clono il segnale di stop per passarlo al task che cattura i frame
    let hotkey_state_clone = Arc::clone(&hotkey_state); //clono hotkey_state per passarlo al task che cattura i frame
    //creazione di un task che gestisce la connessione TCP, rimeanendo in ascolto di nuovi reciver che si connettono
    tokio::spawn(async move {
        while !stop_signal_clone.load(Ordering::SeqCst) && !hotkey_state_clone.terminate.load(Ordering::SeqCst) {
            if let Ok((mut socket, addr)) = listener.accept().await {// accetto la connessione di un reciver
                println!("Nuova connessione da: {}", addr);
                let mut rx = tx_clone.subscribe(); //creazione di un receiver per ricevere i frame da inviare al reciver
                let stop_signal_client = Arc::clone(&stop_signal_clone); //clono il segnale di stop per passarlo al task che invia i frame al reciver
                let hotkey_state_client = Arc::clone(&hotkey_state_clone);//clono hotkey_state per passarlo al task che invia i frame al reciver
                //viene creato un task per ogni reciver che si connette, che invia i frame ricevuti da rx al reciver(rx comunica con tx facendo parte del canale di brodcast)
                tokio::spawn(async move {
                    while !stop_signal_client.load(Ordering::SeqCst) && !hotkey_state_client.terminate.load(Ordering::SeqCst) {
                        match rx.recv().await { //attendo la ricezione di un frame dal canale, se non ricevo nulla mi blocco
                            Ok(frame) => {
                                if frame.len() < 4 {
                                    eprintln!("Errore: frame troppo piccolo per contenere la dimensione.");
                                    break;
                                }
                                let frame_size_bytes = &frame[0..4]; // estraggo i primi 4 byte che contengono la dimensione del frame
                                let frame_data = &frame[4..]; // estraggo il resto del frame
                                if socket.write_all(&frame_size_bytes).await.is_err() || // invio la dimensione del frame al reciver .write_all è bloccante e scirve tutti i byte nel buffer fino a quando non sono stati scritti tutti
                                    socket.write_all(&frame_data).await.is_err() { // invio il frame al reciver
                                    eprintln!("Errore nell'invio del frame al client {}", addr);
                                    break;
                                }
                            }
                            Err(e) => {
                                if e.to_string().contains("lagged") {
                                    eprintln!("Avviso: il canale è in ritardo, salto alcuni frame: {}", e);
                                    continue; // Continue instead of breaking
                                }
                                eprintln!("Errore nella ricezione del frame dal canale: {}", e);
                                break;
                            }
                        }
                    }
                    println!("Connessione chiusa con {}", addr);
                });
            }
        }

        // Invia un segnale esplicito di chiusura ai receiver
        if let Err(_) = tx_clone.send(vec![0, 0, 0, 0]) {
            eprintln!("Errore nell'invio del segnale di terminazione.");
        }

        println!("Listener TCP interrotto.");
    });

    capture_screen(&*Arc::clone(&tx), stop_signal, selected_area, Arc::clone(&hotkey_state),display_index).await?;
    println!("Caster completamente fermato.");
    hotkey_state.screen_blanked.store(false, Ordering::SeqCst);
    hotkey_state.paused.store(false, Ordering::SeqCst);
    hotkey_state.terminate.store(false, Ordering::SeqCst);
    Ok(())
}
