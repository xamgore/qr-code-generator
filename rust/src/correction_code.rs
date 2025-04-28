/// The error correction level in a QR Code symbol.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum QrCodeEcc {
    /// The QR Code can tolerate about  7% erroneous codewords.
    Low,
    /// The QR Code can tolerate about 15% erroneous codewords.
    Medium,
    /// The QR Code can tolerate about 25% erroneous codewords.
    Quartile,
    /// The QR Code can tolerate about 30% erroneous codewords.
    High,
}

impl QrCodeEcc {
    /// Returns an unsigned 2-bit integer (in the range 0 to 3).
    pub(crate) fn ordinal(self) -> usize {
        use QrCodeEcc::*;
        match self {
            Low => 0,
            Medium => 1,
            Quartile => 2,
            High => 3,
        }
    }

    /// Returns an unsigned 2-bit integer (in the range 0 to 3).
    pub(crate) fn format_bits(self) -> u8 {
        use QrCodeEcc::*;
        match self {
            Low => 1,
            Medium => 0,
            Quartile => 3,
            High => 2,
        }
    }
}
