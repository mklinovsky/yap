use std::sync::{mpsc, Mutex};

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use tauri::{AppHandle, Runtime};

use crate::dictation::Paster;

pub struct ClipboardPaster<R: Runtime> {
    app: AppHandle<R>,
    // On X11/Wayland the clipboard content is served by its owner, so it must outlive the paste.
    clipboard: Mutex<Option<arboard::Clipboard>>,
}

impl<R: Runtime> ClipboardPaster<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self {
            app,
            clipboard: Mutex::new(None),
        }
    }

    pub fn copy(&self, text: &str) -> Result<(), String> {
        let mut clipboard = self.clipboard.lock().unwrap_or_else(|e| e.into_inner());
        if clipboard.is_none() {
            *clipboard = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
        }
        clipboard
            .as_mut()
            .expect("clipboard initialized above")
            .set_text(text)
            .map_err(|e| e.to_string())
    }
}

impl<R: Runtime> Paster for ClipboardPaster<R> {
    fn paste(&self, text: &str) -> Result<(), String> {
        self.copy(text)?;
        let (tx, rx) = mpsc::channel();
        // macOS keyboard layout lookups inside enigo must run on the main thread.
        self.app
            .run_on_main_thread(move || {
                let _ = tx.send(press_paste());
            })
            .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?
    }
}

fn press_paste() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    let modifier = if cfg!(target_os = "macos") {
        Key::Meta
    } else {
        Key::Control
    };
    enigo
        .key(modifier, Direction::Press)
        .and_then(|()| enigo.key(Key::Unicode('v'), Direction::Click))
        .and_then(|()| enigo.key(modifier, Direction::Release))
        .map_err(|e| e.to_string())
}
