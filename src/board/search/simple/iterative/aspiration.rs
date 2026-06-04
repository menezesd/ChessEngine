use super::SCORE_INFINITE;

const DELTA_SHALLOW: i32 = 35;
const DELTA_DEEP: i32 = 20;
const SHALLOW_DEPTH_LIMIT: u32 = 5;
const MAX_DELTA: i32 = 800;
const FAIL_HIGH_GROWTH_NUMERATOR: i32 = 3;
const FAIL_HIGH_GROWTH_DENOMINATOR: i32 = 2;
const FAIL_LOW_GROWTH_MULTIPLIER: i32 = 2;

pub(super) struct AspirationWindow {
    alpha: i32,
    beta: i32,
    delta: i32,
}

impl AspirationWindow {
    pub(super) fn new(depth: u32, score: i32) -> Self {
        let delta = if depth <= SHALLOW_DEPTH_LIMIT {
            DELTA_SHALLOW
        } else {
            DELTA_DEEP
        };
        Self {
            alpha: score.saturating_sub(delta),
            beta: score.saturating_add(delta),
            delta,
        }
    }

    pub(super) fn alpha(&self) -> i32 {
        self.alpha
    }

    pub(super) fn beta(&self) -> i32 {
        self.beta
    }

    pub(super) fn fail_high(&mut self) {
        self.beta = self.beta.saturating_add(self.delta);
        self.delta =
            self.delta.saturating_mul(FAIL_HIGH_GROWTH_NUMERATOR) / FAIL_HIGH_GROWTH_DENOMINATOR;
    }

    pub(super) fn fail_low(&mut self) {
        self.alpha = self.alpha.saturating_sub(self.delta);
        self.delta = self.delta.saturating_mul(FAIL_LOW_GROWTH_MULTIPLIER);
    }

    pub(super) fn use_full_window_if_needed(&mut self) {
        if self.delta > MAX_DELTA {
            self.alpha = -SCORE_INFINITE;
            self.beta = SCORE_INFINITE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AspirationWindow, SCORE_INFINITE};

    #[test]
    fn new_uses_shallow_delta_through_depth_five() {
        let window = AspirationWindow::new(5, 100);
        assert_eq!(window.alpha(), 65);
        assert_eq!(window.beta(), 135);
    }

    #[test]
    fn new_uses_deep_delta_after_depth_five() {
        let window = AspirationWindow::new(6, 100);
        assert_eq!(window.alpha(), 80);
        assert_eq!(window.beta(), 120);
    }

    #[test]
    fn fail_high_expands_beta_and_grows_delta() {
        let mut window = AspirationWindow::new(6, 100);
        window.fail_high();
        assert_eq!(window.beta(), 140);
        window.fail_high();
        assert_eq!(window.beta(), 170);
    }

    #[test]
    fn fail_low_expands_alpha_and_grows_delta() {
        let mut window = AspirationWindow::new(6, 100);
        window.fail_low();
        assert_eq!(window.alpha(), 60);
        window.fail_low();
        assert_eq!(window.alpha(), 20);
    }

    #[test]
    fn uses_full_window_after_large_delta() {
        let mut window = AspirationWindow::new(6, 100);
        for _ in 0..6 {
            window.fail_low();
        }
        window.use_full_window_if_needed();
        assert_eq!(window.alpha(), -SCORE_INFINITE);
        assert_eq!(window.beta(), SCORE_INFINITE);
    }
}
