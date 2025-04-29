use std::cmp::Ordering;
use std::ops::{Add, AddAssign, Deref};

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
        let self1 = Version::MAX;
        let self2 = Version::MIN;
        assert!(self2.0 <= ver && ver <= self1.0, "Version number out of range");
        Self(ver)
    }
}

impl Deref for Version {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AddAssign<u8> for Version {
    fn add_assign(&mut self, rhs: u8) {
        self.0 += rhs;
    }
}

impl Add<u8> for Version {
    type Output = Version;

    fn add(mut self, rhs: u8) -> Self::Output {
        self += rhs;
        self
    }
}

impl PartialEq<u8> for Version {
    fn eq(&self, other: &u8) -> bool {
        self.0.eq(other)
    }
}

impl PartialOrd<u8> for Version {
    fn partial_cmp(&self, other: &u8) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl From<Version> for usize {
    fn from(value: Version) -> Self {
        value.0 as usize
    }
}

impl From<Version> for u32 {
    fn from(value: Version) -> Self {
        value.0 as u32
    }
}

impl From<Version> for i32 {
    fn from(value: Version) -> Self {
        value.0 as i32
    }
}

impl From<Version> for u8 {
    fn from(value: Version) -> Self {
        value.0
    }
}
