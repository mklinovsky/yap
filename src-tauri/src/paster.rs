use std::sync::{mpsc, Mutex, MutexGuard};
use std::time::Duration;

use arboard::{Clipboard, ImageData};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use tauri::{AppHandle, Runtime};

use crate::dictation::Paster;

// The target app reads the clipboard asynchronously after Cmd/Ctrl+V; restoring sooner pastes the old content.
const RESTORE_DELAY: Duration = Duration::from_millis(300);

pub struct ClipboardPaster<R: Runtime> {
    app: AppHandle<R>,
    // On X11/Wayland the clipboard content is served by its owner, so it must outlive the paste.
    clipboard: Mutex<Option<Clipboard>>,
}

enum Saved {
    Text(String),
    Image(ImageData<'static>),
}

impl<R: Runtime> ClipboardPaster<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self {
            app,
            clipboard: Mutex::new(None),
        }
    }

    pub fn copy(&self, text: &str) -> Result<(), String> {
        self.clipboard()?
            .as_mut()
            .expect("clipboard initialized")
            .set_text(text)
            .map_err(|e| e.to_string())
    }

    fn clipboard(&self) -> Result<MutexGuard<'_, Option<Clipboard>>, String> {
        let mut clipboard = self.clipboard.lock().unwrap_or_else(|e| e.into_inner());
        if clipboard.is_none() {
            *clipboard = Some(Clipboard::new().map_err(|e| e.to_string())?);
        }
        Ok(clipboard)
    }

    fn press_paste(&self) -> Result<(), String> {
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

impl<R: Runtime> Paster for ClipboardPaster<R> {
    fn paste(&self, text: &str) -> Result<(), String> {
        let mut guard = self.clipboard()?;
        let clipboard = guard.as_mut().expect("clipboard initialized");
        let saved = save(clipboard);
        clipboard.set_text(text).map_err(|e| e.to_string())?;
        let pasted = self.press_paste();
        std::thread::sleep(RESTORE_DELAY);
        // Something copied in the meantime wins over the old content.
        if clipboard.get_text().is_ok_and(|current| current == text) {
            restore(clipboard, saved);
        }
        pasted
    }
}

// Only text and images can be read back; anything else (e.g. copied files) is left replaced by the transcript.
fn save(clipboard: &mut Clipboard) -> Option<Saved> {
    if let Ok(text) = clipboard.get_text() {
        return Some(Saved::Text(text));
    }
    clipboard.get_image().ok().map(Saved::Image)
}

fn restore(clipboard: &mut Clipboard, saved: Option<Saved>) {
    let _ = match saved {
        Some(Saved::Text(text)) => clipboard.set_text(text),
        Some(Saved::Image(image)) => clipboard.set_image(image),
        None => Ok(()),
    };
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
