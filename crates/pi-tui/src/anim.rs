pub struct Spinner {
    frames: &'static [&'static str],
    index: usize,
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl Spinner {
    pub fn new() -> Self {
        Self {
            frames: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            index: 0,
        }
    }

    pub fn tick(&mut self) -> &'static str {
        let frame = self.frames[self.index % self.frames.len()];
        self.index = (self.index + 1) % self.frames.len();
        frame
    }
}
