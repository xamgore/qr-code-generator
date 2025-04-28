/// The error type when the supplied data does not fit any QR Code version.
///
/// Ways to handle this exception include:
///
/// - Decrease the error correction level if it was greater than `QrCodeEcc::Low`.
/// - If the `encode_segments_advanced()` function was called, then increase the maxversion
///   argument if it was less than `Version::MAX`. (This advice does not apply to the
///   other factory functions because they search all versions up to `Version::MAX`.)
/// - Split the text data into better or optimal segments in order to reduce the number of bits required.
/// - Change the text or binary data to be shorter.
/// - Change the text to fit the character set of a particular segment mode (e.g. alphanumeric).
/// - Propagate the error upward to the caller/user.
#[derive(Debug, Clone)]
pub enum DataTooLong {
    SegmentTooLong,
    DataOverCapacity(usize, usize),
}

impl std::error::Error for DataTooLong {}

impl std::fmt::Display for DataTooLong {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Self::SegmentTooLong => write!(f, "Segment too long"),
            Self::DataOverCapacity(data_len, max_capacity) => {
                write!(
                    f,
                    "Data length = {} bits, Max capacity = {} bits",
                    data_len, max_capacity
                )
            }
        }
    }
}
