// ─────────────────────────────────────────────
// Error
// ─────────────────────────────────────────────

/// Error type for [`Confirmator`] operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmatorError {
    NegativeConfirmation,
}

// ─────────────────────────────────────────────
// Trait
// ─────────────────────────────────────────────

/// Trait bound for types supported by Confirmator and ErrorConfirmator.
pub trait ConfirmatorValue:
    Copy + Default + PartialOrd 
{
    fn zero() -> Self;
    fn one() -> Self;
    fn saturating_add(self, rhs: Self) -> Self;
}

impl ConfirmatorValue for u8 {
    fn zero() -> Self { 0 }   fn one() -> Self { 1 }
    fn saturating_add(self, rhs: Self) -> Self { u8::saturating_add(self, rhs) }
}
impl ConfirmatorValue for u16 {
    fn zero() -> Self { 0 }   fn one() -> Self { 1 }
    fn saturating_add(self, rhs: Self) -> Self { u16::saturating_add(self, rhs) }
}
impl ConfirmatorValue for u32 {
    fn zero() -> Self { 0 }   fn one() -> Self { 1 }
    fn saturating_add(self, rhs: Self) -> Self { u32::saturating_add(self, rhs) }
}
impl ConfirmatorValue for f32 {
    fn zero() -> Self { 0.0 } fn one() -> Self { 1.0 }
    fn saturating_add(self, rhs: Self) -> Self { self + rhs }
}

// ─────────────────────────────────────────────
// Confirmator
// ─────────────────────────────────────────────

/// Confirms a condition by accumulating consecutive true steps.
///
/// - Each `true` tick adds `step` to `debounce_counter`, saturating at `confirmation`.
/// - Any `false` tick resets `debounce_counter` to zero.
/// - Returns `Ok(true)` once `debounce_counter == confirmation`, `Ok(false)` otherwise.
/// - Returns `condition` when `confirmation` is 0
/// - Returns `Err(ConfirmatorError::NegativeConfirmation)` if `confirmation` is negative.
///
/// # Example
/// ```
/// # use confirmator::Confirmator;
/// let mut c = Confirmator::<u32>::new();
/// assert!(!c.step(true, 1, 3).unwrap());  // 1
/// assert!(!c.step(true, 1, 3).unwrap());  // 2
/// assert!( c.step(true, 1, 3).unwrap());  // 3 → confirmed
/// c.step(false, 1, 3);           // reset
/// assert_eq!(c.debounce_counter(), 0);
/// ```
pub struct Confirmator<T: ConfirmatorValue> {
    debounce_counter: T,
}

impl<T: ConfirmatorValue> Confirmator<T> {
    /// Creates a new `Confirmator` with `debounce_counter` at zero.
    ///
    /// # Example
    /// ```
    /// # use confirmator::Confirmator;
    /// let c = Confirmator::<u32>::new();
    /// assert_eq!(c.debounce_counter(), 0);
    /// ```
    pub fn new() -> Self {
        Self { debounce_counter: T::zero() }
    }

    /// Advance or reset the confirmator.
    ///
    /// # Arguments
    /// * `condition`    - The boolean signal to confirm.
    /// * `step`         - Progress added per true tick.
    /// * `confirmation` - Threshold that must be reached to confirm. `0` for immediateness.
    ///
    /// # Returns
    /// * `Err(ConfirmatorError::NegativeConfirmation)` if `confirmation` is negative.
    /// * `Ok(true/false)` otherwise:
    ///   - when `confirmation` is zero: `true` if `condition` is true, `false` otherwise.
    ///   - `true` if `debounce_counter >= confirmation`, `false` otherwise.
    ///
    /// # Example
    /// ```
    /// # use confirmator::Confirmator;
    /// let mut c = Confirmator::<u32>::new();
    /// // custom step of 2, confirmation of 4
    /// assert!(!c.step(true, 2, 4).unwrap()); // counter = 2
    /// assert!( c.step(true, 2, 4).unwrap()); // counter = 4 → confirmed
    /// assert!(!c.step(false, 2, 4).unwrap()); // reset → counter = 0
    /// assert_eq!(c.debounce_counter(), 0);
    /// assert!(c.step(true, 1, 0).unwrap());
    /// ```
    pub fn step(&mut self, condition: bool, step: T, confirmation: T) -> Result<bool, ConfirmatorError> {
        if confirmation < T::zero() {
            return Err(ConfirmatorError::NegativeConfirmation);
        }
        if T::zero() == confirmation {
            return Ok(condition);
        }
        if condition {
            if self.debounce_counter < confirmation {
                self.debounce_counter = self.debounce_counter.saturating_add(step);
                if self.debounce_counter >= confirmation {
                    self.debounce_counter = confirmation;
                }
            }
            // do nothing already confirmed
        } else {
            self.reset();
        }
        Ok(self.debounce_counter == confirmation)
    }

    /// Reset internal debounce_counter to zero.
    ///
    /// # Example
    /// ```
    /// # use confirmator::Confirmator;
    /// let mut c = Confirmator::<u32>::new();
    /// c.step(true, 1, 5);
    /// assert_ne!(c.debounce_counter(), 0);
    /// c.reset();
    /// assert_eq!(c.debounce_counter(), 0);
    /// ```
    pub fn reset(&mut self) {
        self.debounce_counter = T::zero();
    }

    /// Current accumulated debounce_counter.
    ///
    /// # Example
    /// ```
    /// # use confirmator::Confirmator;
    /// let mut c = Confirmator::<u32>::new();
    /// c.step(true, 3, 10);
    /// assert_eq!(c.debounce_counter(), 3);
    /// ```
    pub fn debounce_counter(&self) -> T {
        self.debounce_counter
    }
}

impl<T: ConfirmatorValue> Default for Confirmator<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Confirmator ──────────────────────────

    #[test]
    fn confirmator_u8_default_step() {
        let mut c: Confirmator<u8> = Confirmator::new();
        for _ in 0..2 {
            assert!(!c.step(true, 1, 3).unwrap());
        }
        assert!( c.step(true, 1, 3).unwrap());
        assert_eq!(c.debounce_counter(), 3);
    }

    #[test]
    fn confirmator_u8_resets_on_false() {
        let mut c: Confirmator<u8> = Confirmator::new();
        assert!(!c.step(true, 1, 3).unwrap());
        assert_ne!(c.debounce_counter(), 0);
        c.step(false, 1, 3);
        assert_eq!(c.debounce_counter(), 0);
    }

    #[test]
    fn confirmator_u8_saturate_to_max() {
        let mut c: Confirmator<u8> = Confirmator::new();
        assert!(c.step(true, u8::MAX, 3).unwrap());
        assert_eq!(c.debounce_counter(), 3);
    }

    #[test]
    fn confirmator_u8_immediateness() {
        let mut c: Confirmator<u8> = Confirmator::new();
        assert!( c.step(true, 1, 0).unwrap());
        assert!(!c.step(false, 1, 0).unwrap());
    }


    #[test]
    fn confirmator_f32_default_step() {
        let mut c: Confirmator<f32> = Confirmator::new();
        assert!(!c.step(true, 1.0, 3.0).unwrap());
        assert!(!c.step(true, 1.0, 3.0).unwrap());
        assert!( c.step(true, 1.0, 3.0).unwrap()); 
    }

    #[test]
    fn confirmator_f32_resets_on_false() {
        let mut c: Confirmator<f32> = Confirmator::new();
        c.step(true, 1.0, 3.0);
        c.step(true, 1.0, 3.0);
        c.step(false, 1.0, 3.0);
        assert_eq!(c.debounce_counter(), 0.0);
        assert!(!c.step(true, 1.0, 3.0).unwrap());
    }

    #[test]
    fn confirmator_f32_immediateness() {
        let mut c: Confirmator<f32> = Confirmator::new();
        assert!( c.step(true, 1.0, 0.0).unwrap());
        assert!(!c.step(false, 1.0, 0.0).unwrap());
    }

    #[test]
    fn confirmator_f32_saturate_to_max() {
        let mut c: Confirmator<f32> = Confirmator::new();
        assert!(c.step(true, f32::MAX, 3.0).unwrap());
        assert_eq!(c.debounce_counter(), 3.0);
    }

    #[test]
    fn confirmator_negative_confirmation() {
        let mut c: Confirmator<f32> = Confirmator::new();
        assert_eq!(c.step(true, 1.0, -1.0), Err(ConfirmatorError::NegativeConfirmation));
    }

}