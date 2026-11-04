use std::sync::{Arc, Mutex};

/// A trait for interacting with a clipboard (either in-memory or system).
pub trait Clipboard: Send + Sync {
    /// Retrieve text from the clipboard.
    fn get(&mut self) -> Option<String>;

    /// Store text in the clipboard.
    fn set(&mut self, text: &str);
}

impl<C: Clipboard + ?Sized> Clipboard for Box<C> {
    fn get(&mut self) -> Option<String> {
        (**self).get()
    }

    fn set(&mut self, text: &str) {
        (**self).set(text);
    }
}

impl<C: Clipboard> Clipboard for Arc<Mutex<C>> {
    fn get(&mut self) -> Option<String> {
        self.lock().ok().and_then(|mut c| c.get())
    }

    fn set(&mut self, text: &str) {
        if let Ok(mut c) = self.lock() {
            c.set(text);
        }
    }
}

/// An in-memory clipboard / kill ring that stores text in a simple buffer.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct InMemoryClipboard {
    content: Option<String>,
}

impl InMemoryClipboard {
    /// Create a new empty in-memory clipboard.
    pub fn new() -> Self {
        Self { content: None }
    }
}

impl Clipboard for InMemoryClipboard {
    fn get(&mut self) -> Option<String> {
        self.content.clone()
    }

    fn set(&mut self, text: &str) {
        self.content = Some(text.to_string());
    }
}

/// A system clipboard that interfaces with the desktop operating system clipboard (Wayland, X11, macOS, Windows).
///
/// Falls back gracefully to an in-memory clipboard if the system clipboard server is unavailable (e.g. headless CI or SSH).
#[cfg(feature = "system-clipboard")]
pub struct SystemClipboard {
    inner: Option<arboard::Clipboard>,
    fallback: InMemoryClipboard,
}

#[cfg(feature = "system-clipboard")]
impl SystemClipboard {
    /// Create a new system clipboard instance.
    pub fn new() -> Self {
        let inner = arboard::Clipboard::new().ok();
        Self {
            inner,
            fallback: InMemoryClipboard::new(),
        }
    }
}

#[cfg(feature = "system-clipboard")]
impl Default for SystemClipboard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "system-clipboard")]
impl Clipboard for SystemClipboard {
    fn get(&mut self) -> Option<String> {
        if let Some(cb) = &mut self.inner
            && let Ok(text) = cb.get_text()
        {
            return Some(text);
        }
        self.fallback.get()
    }

    fn set(&mut self, text: &str) {
        if let Some(cb) = &mut self.inner {
            let _ = cb.set_text(text);
        }
        self.fallback.set(text);
    }
}

/// Creates the default clipboard for the editor:
/// `SystemClipboard` if the `system-clipboard` feature is enabled, otherwise `InMemoryClipboard`.
pub fn default_clipboard() -> Box<dyn Clipboard> {
    #[cfg(feature = "system-clipboard")]
    {
        Box::new(SystemClipboard::new())
    }
    #[cfg(not(feature = "system-clipboard"))]
    {
        Box::new(InMemoryClipboard::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_clipboard() {
        let mut cb = InMemoryClipboard::new();
        assert_eq!(cb.get(), None);

        cb.set("hello clipboard");
        assert_eq!(cb.get().as_deref(), Some("hello clipboard"));

        cb.set("another text");
        assert_eq!(cb.get().as_deref(), Some("another text"));
    }

    #[test]
    fn test_arc_mutex_clipboard() {
        let cb = Arc::new(Mutex::new(InMemoryClipboard::new()));
        let mut cloned = Arc::clone(&cb);

        assert_eq!(cloned.get(), None);
        cloned.set("shared text");
        assert_eq!(cloned.get().as_deref(), Some("shared text"));
    }

    #[test]
    fn test_default_clipboard_creation() {
        let mut cb = default_clipboard();
        cb.set("test default");
        assert_eq!(cb.get().as_deref(), Some("test default"));
    }
}
