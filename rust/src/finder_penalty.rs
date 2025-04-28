pub struct FinderPenalty {
    qr_size: i32,
    run_history: [i32; 7],
}

impl FinderPenalty {
    pub fn new(size: i32) -> Self {
        Self {
            qr_size: size,
            run_history: [0i32; 7],
        }
    }

    /// Pushes the given value to the front and drops the last value.
    pub fn add_history(&mut self, mut current_run_length: i32) {
        if self.run_history[0] == 0 {
            current_run_length += self.qr_size; // Add light border to initial run
        }
        let rh = &mut self.run_history;
        for i in (0..rh.len() - 1).rev() {
            rh[i + 1] = rh[i];
        }
        rh[0] = current_run_length;
    }

    /// Can only be called immediately after a light run is added, and returns either 0, 1, or 2.
    pub fn count_patterns(&self) -> i32 {
        let rh = &self.run_history;
        let n = rh[1];
        debug_assert!(n <= self.qr_size * 3);
        let core = n > 0 && rh[2] == n && rh[3] == n * 3 && rh[4] == n && rh[5] == n;
        #[allow(unused_parens)]
        (i32::from(core && rh[0] >= n * 4 && rh[6] >= n) + i32::from(core && rh[6] >= n * 4 && rh[0] >= n))
    }

    /// Must be called at the end of a line (row or column) of modules.
    pub fn terminate_and_count(mut self, current_run_color: bool, mut current_run_length: i32) -> i32 {
        if current_run_color {
            // Terminate dark run
            self.add_history(current_run_length);
            current_run_length = 0;
        }
        current_run_length += self.qr_size; // Add light border to final run
        self.add_history(current_run_length);
        self.count_patterns()
    }
}
