/// A number between 1 and 40 (inclusive).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Version(u8);

impl Version {
    /// The minimum version number supported in the QR Code Model 2 standard.
    pub const MIN: Version = Version(1);

    /// The maximum version number supported in the QR Code Model 2 standard.
    pub const MAX: Version = Version(40);

    /// Creates a version object from the given number.
    ///
    /// Panics if the number is outside the range [1, 40].
    pub const fn new(ver: u8) -> Self {
        assert!(
            Version::MIN.value() <= ver && ver <= Version::MAX.value(),
            "Version number out of range"
        );
        Self(ver)
    }

    /// Returns the value, which is in the range [1, 40].
    pub const fn value(self) -> u8 {
        self.0
    }
}
