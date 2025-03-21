# Multi-platform Screencasting Application

## Overview
This is a multi-platform screencasting application developed in Rust, supporting Windows, macOS, and Linux. The software enables real-time screen sharing with multiple peers, offering an intuitive and user-friendly experience. Users can act as transmitters or receivers, select portions of the screen to share, and control the transmission via customizable keyboard shortcuts.

## Key Features
- Compatible with Windows, macOS and Linux
- Caster and receiver modes
- Possibility to select specific screen areas to share
- Customizable keyboard shortcuts for transmission control
- Annotation toolbar designed for educational purposes:
  - Rectangles: for highlighting areas
  - Arrows: for pointing to specific elements
  - Text: tool for adding explanations

## Installation
Ensure you have the following installed:
- Rust
- RustRover (recommended IDE for Rust development)
- All required dependencies using Cargo

To enable screen recording you need to install FFmpeg. 
Run the following commands based on your operating system:
- **Windows**:
  - `irm get.scoop.sh | iex`
  - `scoop install ffmpeg`
- **macOS**:
  - `/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"`
  - `brew install ffmpeg`
- **Linux**:
  - `sudo apt update`
  - `sudo apt install -y ffmpeg`


## Project Structure
- **src/main.rs:** Application entry point (main logic) 
- **src/caster.rs:** Handles screen capture and transmission
- **src/receiver.rs:** Handles screen reception and display
- **src/ui.rs:** Manages the user interface and toolbar

## Usage
1. Launch the application with `cargo run --release ui`
2. Select whether to transmit or receive a screen
3. If transmitting, choose the screen area to share
4. Use keyboard shortcuts to pause/resume, blank or stop transmission
5. Peers can connect and view the shared screen in real-time

## Keyboard Shortcuts
- **Fn + F1**: Pause/Resume Transmission
- **Fn + F2**: Blank Screen
- **ESC**: Stop Transmission

## Configuration
The application supports configuration via a settings file `config.toml`

## Possible improvements
- Improve recording so that it is not necessary to save all frames before converting them to video
- Use `tokio::spawn` instead of `std::thread::spawn` when creating threads for managing the caster and receiver
