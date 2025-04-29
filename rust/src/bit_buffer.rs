use std::ops::{Deref, DerefMut};

use crate::get_bit;

/// An appendable sequence of bits (0s and 1s).
///
/// Mainly used by QrSegment.
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct BitBuffer(Vec<bool>);

impl BitBuffer {
    /// Appends the given number of low-order bits of the given value to this buffer.
    ///
    /// Requires len &#x2264; 31 and val &lt; 2<sup>len</sup>.
    pub fn append_bits(&mut self, val: u32, len: u8) {
        assert!(len <= 31 && val >> len == 0, "Value out of range");
        self.0.extend((0..i32::from(len)).rev().map(|i| get_bit(val, i))); // Append bit by bit
    }

    pub fn into_inner(self) -> Vec<bool> {
        self.0
    }
}

impl From<Vec<bool>> for BitBuffer {
    fn from(value: Vec<bool>) -> Self {
        BitBuffer(value)
    }
}

impl Deref for BitBuffer {
    type Target = Vec<bool>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BitBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
