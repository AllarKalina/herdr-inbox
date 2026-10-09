//! The one-line message above the shortcut line.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tone {
    Info,
    Success,
    Error,
}

/// The one-line message shown above the shortcut line until the next action replaces it.
pub(super) struct Notice {
    text: String,
    tone: Tone,
}

impl Default for Notice {
    fn default() -> Self {
        Self {
            text: String::new(),
            tone: Tone::Info,
        }
    }
}

impl Notice {
    fn set(&mut self, text: impl Into<String>, tone: Tone) {
        self.text = text.into();
        self.tone = tone;
    }

    pub(super) fn info(&mut self, text: impl Into<String>) {
        self.set(text, Tone::Info);
    }

    pub(super) fn success(&mut self, text: impl Into<String>) {
        self.set(text, Tone::Success);
    }

    pub(super) fn error(&mut self, text: impl Into<String>) {
        self.set(text, Tone::Error);
    }

    pub(super) fn clear(&mut self) {
        self.text.clear();
    }

    pub(super) fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }

    pub(super) fn tone(&self) -> Tone {
        self.tone
    }
}
