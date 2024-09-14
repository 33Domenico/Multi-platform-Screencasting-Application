use crate::caster::capture_screen;

mod caster;
mod receiver;
mod ui;
mod network;

fn main() {
    if let Err(e) = capture_screen() {
        eprintln!("Error during screen capture: {}", e);
    }
}
