use typed_floats::tf32::Positive;

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
impl ConfirmatorValue for Positive {
    fn zero() -> Self { 0.0.try_into().unwrap() } fn one() -> Self { 1.0.try_into().unwrap() }
    fn saturating_add(self, rhs: Self) -> Self { (self + rhs).min( f32::MAX.try_into().unwrap()) }
}

// ─────────────────────────────────────────────
// Confirmator
// ─────────────────────────────────────────────

/// Confirms a condition by accumulating consecutive true steps.
///
/// - Each `true` tick adds `step` to `debounce_counter`, saturating at `confirmation`.
/// - Any `false` tick resets `debounce_counter` to zero.
/// - Returns `true` once `debounce_counter == confirmation`, `false` otherwise.
/// - Returns `condition` when `confirmation` is 0
///
/// # Example
/// ```
/// # use confirmator::Confirmator;
/// let mut c = Confirmator::<u32>::new();
/// assert!(!c.step(true, None, 3));  // 1
/// assert!(!c.step(true, None, 3));  // 2
/// assert!( c.step(true, None, 3));  // 3 → confirmed
/// c.step(false, None, 3);           // reset
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
    /// * `step`         - Progress added per true tick. Defaults to `1` if `None`.
    /// * `confirmation` - Threshold that must be reached to confirm. `0` for immediateness.
    ///
    /// # Returns
    /// when `confirmation` is zero
    /// `true` if `condition` is true, `false` otherwise.
    /// `true` if `debounce_counter >= confirmation`, `false` otherwise.
    ///
    /// # Example
    /// ```
    /// # use confirmator::Confirmator;
    /// let mut c = Confirmator::<u32>::new();
    /// // custom step of 2, confirmation of 4
    /// assert!(!c.step(true, Some(2), 4)); // counter = 2
    /// assert!( c.step(true, Some(2), 4)); // counter = 4 → confirmed
    /// assert!(!c.step(false, Some(2), 4)); // reset → counter = 0
    /// assert_eq!(c.debounce_counter(), 0);
    /// assert!(c.step(true, None, 0));
    /// ```
    pub fn step(&mut self, condition: bool, step: Option<T>, confirmation: T) -> bool {
        let step = step.unwrap_or_else(T::one);
        if T::zero() == confirmation {
            return condition;
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
        self.debounce_counter == confirmation
    }

    /// Reset internal debounce_counter to zero.
    ///
    /// # Example
    /// ```
    /// # use confirmator::Confirmator;
    /// let mut c = Confirmator::<u32>::new();
    /// c.step(true, None, 5);
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
    /// c.step(true, Some(3), 10);
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
    use typed_floats::tf32::ZERO;

    use super::*;

    // ── Confirmator ──────────────────────────

    #[test]
    fn confirmator_u8_default_step() {
        let mut c: Confirmator<u8> = Confirmator::new();
        for _ in 0..2 {
            assert!(!c.step(true, None, 3));
        }
        assert!( c.step(true, None, 3));
        assert_eq!(c.debounce_counter(), 3);
    }

    #[test]
    fn confirmator_u8_resets_on_false() {
        let mut c: Confirmator<u8> = Confirmator::new();
        assert!(!c.step(true, None, 3));
        assert_ne!(c.debounce_counter(), 0);
        c.step(false, None, 3);
        assert_eq!(c.debounce_counter(), 0);
    }

    #[test]
    fn confirmator_u8_saturate_to_max() {
        let mut c: Confirmator<u8> = Confirmator::new();
        assert!(c.step(true, Some(u8::MAX), 3));
        assert_eq!(c.debounce_counter(), 3);
    }

    #[test]
    fn confirmator_u8_immediateness() {
        let mut c: Confirmator<u8> = Confirmator::new();
        assert!( c.step(true, None, 0));
        assert!(!c.step(false, None, 0));
    }


    #[test]
    fn confirmator_tf32_default_step() {
        let mut c: Confirmator<Positive> = Confirmator::new();
        assert!(!c.step(true, None, 3.0.try_into().unwrap()));
        assert!(!c.step(true, None, 3.0.try_into().unwrap()));
        assert!( c.step(true, None, 3.0.try_into().unwrap())); 
    }

    #[test]
    fn confirmator_tf32_resets_on_false() {
        let mut c: Confirmator<Positive> = Confirmator::new();
        c.step(true, None, 3.0.try_into().unwrap());
        c.step(true, None, 3.0.try_into().unwrap());
        c.step(false, None, 3.0.try_into().unwrap());
        assert_eq!(c.debounce_counter(), ZERO);
        assert!(!c.step(true, None, 3.0.try_into().unwrap()));
    }

    #[test]
    fn confirmator_tf32_immediateness() {
        let mut c: Confirmator<Positive> = Confirmator::new();
        assert!( c.step(true, None, 0.0.try_into().unwrap()));
        assert!(!c.step(false, None, 0.0.try_into().unwrap()));
    }

    #[test]
    fn confirmator_tf32_saturate_to_max() {
        let mut c: Confirmator<Positive> = Confirmator::new();
        assert!(c.step(true, Some(f32::MAX.try_into().unwrap()), 3.0.try_into().unwrap()));
        assert_eq!(c.debounce_counter(), 3.0f32);
    }

}