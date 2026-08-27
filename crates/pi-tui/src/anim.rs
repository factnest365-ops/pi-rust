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
            // Premium 10-frame braille — smooth, consistent weight, no jump (G2-like)
            frames: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            index: 0,
        }
    }

    pub fn tick(&mut self) -> &'static str {
        let frame = self.frames[self.index % self.frames.len()];
        self.index = (self.index + 1) % self.frames.len();
        frame
    }

    /// Premium variant: denser braille for high-salience contexts (tool execution)
    pub fn tick_dense(&mut self) -> &'static str {
        const DENSE: &[&str] = &["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];
        let frame = DENSE[self.index % DENSE.len()];
        self.index = (self.index + 1) % DENSE.len();
        frame
    }
}
