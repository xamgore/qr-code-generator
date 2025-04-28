/// A number between 0 and 7 (inclusive).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Mask(u8);

impl Mask {
    /// Creates a mask object from the given number.
    ///
    /// Panics if the number is outside the range [0, 7].
    pub const fn new(mask: u8) -> Self {
        assert!(mask <= 7, "Mask value out of range");
        Self(mask)
    }

    /// Returns the value, which is in the range [0, 7].
    pub const fn value(self) -> u8 {
        self.0
    }
}
