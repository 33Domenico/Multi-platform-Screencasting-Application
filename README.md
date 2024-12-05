# rustproject
A screencasting application capable of continuously  grabbing the content of the screen (or a portion of it) and stream it to a set of peers with Rust Programming Language 

Struttura del progetto

src/main.rs: Punto di ingresso dell'applicazione, dove puoi gestire l'interfaccia utente e le opzioni.
src/caster.rs: Modulo che gestisce la modalità caster (cattura dello schermo e invio).
src/receiver.rs: Modulo che gestisce la modalità receiver (ricezione e visualizzazione).
src/ui.rs: Modulo per gestire l'interfaccia utente.

Per la funzione registrazione è necessario installare una libreria esterna, digitando da terminale i seguenti comandi:

Per Windows:

irm get.scoop.sh | iex

scoop install ffmpeg 

Per macOS:
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
brew install ffmpeg

Per Linux su Debian/Ubuntu:
sudo apt update && sudo apt install -y ffmpeg

Per Linux su Fedora:
sudo dnf install -y ffmpeg

Per Linux su Arch Linux:
sudo pacman -Syu ffmpeg


