use crate::dictation::SelectionReader;

pub struct SystemSelectionReader;

impl SelectionReader for SystemSelectionReader {
    fn selected_text(&self) -> Result<Option<String>, String> {
        #[cfg(target_os = "macos")]
        {
            objc2::rc::autoreleasepool(|_| match macos::selected_text()? {
                Some(text) => Ok(Some(text)),
                None => copy_capture::capture(&macos::ClipboardCopy::new()),
            })
        }
        #[cfg(not(target_os = "macos"))]
        {
            Ok(None)
        }
    }
}

// Prefer the focused element directly; use Copy only when Accessibility supplies no text.
// https://developer.apple.com/documentation/applicationservices/1462085-axuielementcopyattributevalue
#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::{c_char, c_void, CStr};
    use std::ptr;
    use std::time::Duration;

    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2_app_kit::{
        NSPasteboard, NSPasteboardItem, NSPasteboardTypeString, NSPasteboardWriting,
    };
    use objc2_foundation::NSArray;

    pub(super) struct ClipboardCopy(Retained<NSPasteboard>);

    impl ClipboardCopy {
        pub(super) fn new() -> Self {
            Self(NSPasteboard::generalPasteboard())
        }
    }

    impl super::copy_capture::ClipboardCopy for ClipboardCopy {
        type Snapshot = Vec<Retained<NSPasteboardItem>>;

        fn change_count(&self) -> isize {
            self.0.changeCount()
        }

        fn snapshot(&self) -> Result<Self::Snapshot, String> {
            let mut saved = Vec::new();
            if let Some(items) = self.0.pasteboardItems() {
                for item in &items {
                    let duplicate = NSPasteboardItem::new();
                    for kind in &item.types() {
                        let data = item.dataForType(&kind).ok_or(
                            "Could not preserve the clipboard before reading selected text.",
                        )?;
                        if !duplicate.setData_forType(&data, &kind) {
                            return Err(
                                "Could not preserve the clipboard before reading selected text."
                                    .into(),
                            );
                        }
                    }
                    saved.push(duplicate);
                }
            }
            Ok(saved)
        }

        fn copy(&self) -> Result<(), String> {
            press_copy()
        }

        fn text(&self) -> Option<String> {
            self.0
                .stringForType(unsafe { NSPasteboardTypeString })
                .map(|text| text.to_string())
        }

        fn restore(&self, saved: Self::Snapshot) -> Result<(), String> {
            self.0.clearContents();
            if !saved.is_empty() {
                let objects: Vec<&ProtocolObject<dyn NSPasteboardWriting>> = saved
                    .iter()
                    .map(|item| ProtocolObject::from_ref(&**item))
                    .collect();
                if !self.0.writeObjects(&NSArray::from_slice(&objects)) {
                    return Err(
                        "Could not restore the clipboard after reading selected text.".into(),
                    );
                }
            }
            Ok(())
        }

        fn wait(&self) {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceCreate(state: i32) -> Ref;
        fn CGEventCreateKeyboardEvent(source: Ref, key: u16, down: bool) -> Ref;
        fn CGEventSetFlags(event: Ref, flags: u64);
        fn CGEventPost(tap: u32, event: Ref);
    }

    fn press_copy() -> Result<(), String> {
        // Use private event state and explicit Command flags: the instruction shortcut's
        // physically held Shift/Option/Control keys must not turn Copy into another command.
        unsafe {
            let source = CGEventSourceCreate(-1);
            if source.is_null() {
                return Err("Could not create the Copy event source.".into());
            }
            let source = Owned(source);
            let down = CGEventCreateKeyboardEvent(source.0, 8, true); // physical C
            if down.is_null() {
                return Err("Could not create the Copy key event.".into());
            }
            let down = Owned(down);
            let up = CGEventCreateKeyboardEvent(source.0, 8, false);
            if up.is_null() {
                return Err("Could not create the Copy key event.".into());
            }
            let up = Owned(up);
            for event in [&down, &up] {
                CGEventSetFlags(event.0, 1 << 20); // Command only
                CGEventPost(1, event.0); // Session event tap; no physical modifier releases
            }
        }
        Ok(())
    }

    type Ref = *const c_void;
    const UTF8: u32 = 0x08000100;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXUIElementCreateSystemWide() -> Ref;
        fn AXUIElementGetTypeID() -> usize;
        fn AXUIElementSetMessagingTimeout(element: Ref, timeout: f32) -> i32;
        fn AXUIElementCopyAttributeValue(element: Ref, attribute: Ref, value: *mut Ref) -> i32;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(value: Ref);
        fn CFGetTypeID(value: Ref) -> usize;
        fn CFStringGetTypeID() -> usize;
        fn CFStringCreateWithBytes(
            allocator: Ref,
            bytes: *const u8,
            length: isize,
            encoding: u32,
            external: bool,
        ) -> Ref;
        fn CFStringGetLength(string: Ref) -> isize;
        fn CFStringGetCString(string: Ref, buffer: *mut c_char, size: isize, encoding: u32)
            -> bool;
    }

    // Own only objects returned by Create/Copy; all live and are released on this worker thread.
    struct Owned(Ref);
    impl Drop for Owned {
        fn drop(&mut self) {
            unsafe { CFRelease(self.0) };
        }
    }

    fn attribute(element: &Owned, name: &str) -> Result<Option<Owned>, String> {
        let name = unsafe {
            CFStringCreateWithBytes(ptr::null(), name.as_ptr(), name.len() as isize, UTF8, false)
        };
        if name.is_null() {
            return Err("Could not create the selected-text attribute.".into());
        }
        let name = Owned(name);
        let mut value = ptr::null();
        let error = unsafe { AXUIElementCopyAttributeValue(element.0, name.0, &mut value) };
        match error {
            0 if !value.is_null() => Ok(Some(Owned(value))),
            // Unsupported attribute / no value: this input exposes no selection.
            0 | -25205 | -25212 => Ok(None),
            _ => Err(format!(
                "Could not read selected text (Accessibility error {error})."
            )),
        }
    }

    pub fn selected_text() -> Result<Option<String>, String> {
        // SAFETY: these C APIs return owned CF objects; types are checked before specialized calls.
        unsafe {
            if !AXIsProcessTrusted() {
                return Err("Allow yap in System Settings → Privacy & Security → Accessibility to read selected text and paste results.".into());
            }
            let system = AXUIElementCreateSystemWide();
            if system.is_null() {
                return Err("Could not access the focused input.".into());
            }
            let system = Owned(system);
            AXUIElementSetMessagingTimeout(system.0, 1.0);
            let Some(focused) = attribute(&system, "AXFocusedUIElement")? else {
                return Ok(None);
            };
            if CFGetTypeID(focused.0) != AXUIElementGetTypeID() {
                return Err("Accessibility returned an invalid focused input.".into());
            }
            let Some(text) = attribute(&focused, "AXSelectedText")? else {
                return Ok(None);
            };
            if CFGetTypeID(text.0) != CFStringGetTypeID() {
                return Err("The selected content is not text.".into());
            }
            let mut buffer = vec![0u8; CFStringGetLength(text.0) as usize * 4 + 1];
            if !CFStringGetCString(
                text.0,
                buffer.as_mut_ptr().cast(),
                buffer.len() as isize,
                UTF8,
            ) {
                return Err("Could not decode selected text.".into());
            }
            let text = CStr::from_ptr(buffer.as_ptr().cast())
                .to_str()
                .map_err(|error| error.to_string())?
                .to_string();
            Ok((!text.trim().is_empty()).then_some(text))
        }
    }
}

#[cfg(any(target_os = "macos", test))]
mod copy_capture {
    pub(super) trait ClipboardCopy {
        type Snapshot;
        fn change_count(&self) -> isize;
        fn snapshot(&self) -> Result<Self::Snapshot, String>;
        fn copy(&self) -> Result<(), String>;
        fn text(&self) -> Option<String>;
        fn restore(&self, snapshot: Self::Snapshot) -> Result<(), String>;
        fn wait(&self);
    }

    pub(super) fn capture<B: ClipboardCopy>(backend: &B) -> Result<Option<String>, String> {
        let before = backend.change_count();
        let saved = backend.snapshot()?;
        if backend.change_count() != before {
            return Err("The clipboard changed while reading selected text. Please retry.".into());
        }
        backend.copy()?;
        // Copy is asynchronous. A changed count is required even if the selection is
        // identical to the old clipboard text; an unchanged clipboard is never context.
        let mut copied_count = None;
        let mut selected = None;
        for _ in 0..50 {
            let count = backend.change_count();
            if count != before {
                copied_count = Some(count);
                selected = backend.text().filter(|text| !text.trim().is_empty());
                if selected.is_some() {
                    break;
                }
            }
            backend.wait();
        }
        if let Some(count) = copied_count {
            if backend.change_count() != count {
                return Err(
                    "The clipboard changed while reading selected text. Please retry.".into(),
                );
            }
            backend.restore(saved)?;
        }
        Ok(selected)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::cell::Cell;

        struct FakeCopy {
            count: Cell<isize>,
            copied: Option<String>,
            restored: Cell<bool>,
            delay: usize,
            waits: Cell<usize>,
            changed_during_read: bool,
        }
        impl FakeCopy {
            fn new(copied: Option<&str>) -> Self {
                Self {
                    count: Cell::new(1),
                    copied: copied.map(String::from),
                    restored: Cell::new(false),
                    delay: 0,
                    waits: Cell::new(0),
                    changed_during_read: false,
                }
            }
        }
        impl ClipboardCopy for FakeCopy {
            type Snapshot = ();
            fn change_count(&self) -> isize {
                self.count.get()
            }
            fn snapshot(&self) -> Result<(), String> {
                Ok(())
            }
            fn copy(&self) -> Result<(), String> {
                if self.copied.is_some() && self.delay == 0 {
                    self.count.set(self.count.get() + 1);
                }
                Ok(())
            }
            fn text(&self) -> Option<String> {
                if self.changed_during_read {
                    self.count.set(self.count.get() + 1);
                }
                self.copied.clone().or(Some("old clipboard".into()))
            }
            fn restore(&self, _: ()) -> Result<(), String> {
                self.restored.set(true);
                Ok(())
            }
            fn wait(&self) {
                self.waits.set(self.waits.get() + 1);
                if self.waits.get() == self.delay && self.copied.is_some() {
                    self.count.set(self.count.get() + 1);
                }
            }
        }

        #[test]
        fn copy_capture_returns_fresh_selection_and_restores_clipboard() {
            let backend = FakeCopy::new(Some("Slack selection"));
            assert_eq!(capture(&backend).unwrap(), Some("Slack selection".into()));
            assert!(backend.restored.get());
        }

        #[test]
        fn unchanged_clipboard_is_never_used_as_selection() {
            let backend = FakeCopy::new(None);
            assert_eq!(capture(&backend).unwrap(), None);
            assert!(!backend.restored.get());
        }

        #[test]
        fn fresh_copy_of_the_same_text_is_still_a_selection() {
            let backend = FakeCopy::new(Some("old clipboard"));
            assert_eq!(capture(&backend).unwrap(), Some("old clipboard".into()));
            assert!(backend.restored.get());
        }

        #[test]
        fn asynchronous_copy_is_awaited() {
            let mut backend = FakeCopy::new(Some("delayed selection"));
            backend.delay = 3;
            assert_eq!(capture(&backend).unwrap(), Some("delayed selection".into()));
            assert_eq!(backend.waits.get(), 3);
            assert!(backend.restored.get());
        }

        #[test]
        fn another_clipboard_change_is_not_overwritten() {
            let mut backend = FakeCopy::new(Some("selection"));
            backend.changed_during_read = true;
            assert!(capture(&backend).is_err());
            assert!(!backend.restored.get());
        }

        #[test]
        fn blank_copy_restores_clipboard_and_provides_no_context() {
            let backend = FakeCopy::new(Some(" "));
            assert_eq!(capture(&backend).unwrap(), None);
            assert!(backend.restored.get());
        }
    }
}
