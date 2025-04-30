use std::convert::TryFrom;

use crate::bit_buffer::BitBuffer;
use crate::correction_code::ErrCorrectLvl;
use crate::error::DataTooLong;
use crate::finder_penalty::FinderPenalty;
use crate::get_bit;
use crate::mask::Mask;
use crate::qr_segment::QrSegment;
use crate::version::Version;

/// A QR Code symbol, which is a type of two-dimension barcode.
///
/// Invented by Denso Wave and described in the ISO/IEC 18004 standard.
///
/// Instances of this struct represent an immutable square grid of dark and light cells.
/// The impl provides static factory functions to create a QR Code from text or binary data.
/// The struct and impl cover the QR Code Model 2 specification, supporting all versions
/// (sizes) from 1 to 40, all 4 error correction levels, and 4 character encoding modes.
///
/// Ways to create a QR Code object:
///
/// - High level: Take the payload data and call [`QrCode::encode_text()`](QrCode::encode_text) or
///   [`QrCode::encode_binary()`](QrCode::encode_binary).
/// - Mid level: Custom-make the list of segments and call
///   [`QrCode::encode_segments()`](QrCode::encode_segments) or
///   [`QrCode::encode_segments_advanced()`](QrCode::encode_segments_advanced).
/// - Low level: Custom-make the array of data codeword bytes (including segment
///   headers and final padding, excluding error correction codewords), supply the
///   appropriate version number, and call the [`QrCode::encode_codewords()`](QrCode::encode_codewords)
///   constructor.
///
/// (Note that each way requires supplying the desired error correction level.)
#[derive(Clone, PartialEq, Eq)]
pub struct QrCode {
    // Scalar parameters:
    /// The version number of this QR Code, which is between 1 and 40 (inclusive).
    /// This determines the size of this barcode.
    version: Version,

    /// The width and height of this QR Code, measured in modules, between
    /// 21 and 177 (inclusive). This is equal to version * 4 + 17.
    size: i32,

    /// The error correction level used in this QR Code.
    error_correction_level: ErrCorrectLvl,

    /// The index of the mask pattern used in this QR Code, which is between 0 and 7 (inclusive).
    /// Even if a QR Code is created with automatic masking requested (mask = None),
    /// the resulting object still has a mask value between 0 and 7.
    mask: Mask,

    // Grids of modules/pixels, with dimensions of size*size:
    /// The modules of this QR Code (false = light, true = dark).
    /// Immutable after constructor finishes. Accessed through get_module().
    modules: Vec<bool>,

    /// Indicates function modules that are not subjected to masking. Discarded when constructor finishes.
    is_function: Vec<bool>,
}

impl QrCode {
    /*---- Static factory functions (high level) ----*/

    /// Returns a QR Code representing the given Unicode text string at the given error correction level.
    ///
    /// As a conservative upper bound, this function is guaranteed to succeed for strings that have 738 or fewer Unicode
    /// code points (not UTF-8 code units) if the low error correction level is used. The smallest possible
    /// QR Code version is automatically chosen for the output. The ECC level of the result may be higher than
    /// the `err_correction_lvl` argument if it can be done without increasing the version.
    ///
    /// Returns a wrapped `QrCode` if successful, or `Err` if the
    /// data is too long to fit in any version at the given ECC level.
    pub fn encode_text(text: &str, err_correction_lvl: ErrCorrectLvl) -> Result<Self, DataTooLong> {
        let segments: Vec<QrSegment> = QrSegment::make_segments(text);
        QrCode::encode_segments(&segments, err_correction_lvl)
    }

    /// Returns a QR Code representing the given binary data at the given error correction level.
    ///
    /// This function always encodes using the binary segment mode, not any text mode. The maximum number of
    /// bytes allowed is 2953. The smallest possible QR Code version is automatically chosen for the output.
    /// The ECC level of the result may be higher than the ecl argument if it can be done without increasing the version.
    ///
    /// Returns a wrapped `QrCode` if successful, or `Err` if the
    /// data is too long to fit in any version at the given ECC level.
    pub fn encode_binary(data: &[u8], ecl: ErrCorrectLvl) -> Result<Self, DataTooLong> {
        let segments: [QrSegment; 1] = [QrSegment::make_bytes(data)];
        QrCode::encode_segments(&segments, ecl)
    }

    /*---- Static factory functions (mid level) ----*/

    /// Returns a QR Code representing the given segments at the given error correction level.
    ///
    /// The smallest possible QR Code version is automatically chosen for the output. The ECC level
    /// of the result may be higher than the ecl argument if it can be done without increasing the version.
    ///
    /// This function allows the user to create a custom sequence of segments that switches
    /// between modes (such as alphanumeric and byte) to encode text in less space.
    /// This is a mid-level API; the high-level API is [`encode_text()`](QrCode::encode_text) and [`encode_binary()`](QrCode::encode_binary).
    ///
    /// Returns a wrapped `QrCode` if successful, or `Err` if the
    /// data is too long to fit in any version at the given ECC level.
    pub fn encode_segments(segments: &[QrSegment], ecl: ErrCorrectLvl) -> Result<Self, DataTooLong> {
        QrCode::encode_segments_advanced(segments, ecl, Version::MIN, Version::MAX, None, true)
    }

    /// Returns a QR Code representing the given segments with the given encoding parameters.
    ///
    /// The smallest possible QR Code version within the given range is automatically
    /// chosen for the output. Iff `boost_ecl` is `true`, then the ECC level of the result
    /// may be higher than the ecl argument if it can be done without increasing the
    /// version. The mask number is either between 0 and 7 (inclusive) to force that
    /// mask, or `None` to automatically choose an appropriate mask (which may be slow).
    ///
    /// This function allows the user to create a custom sequence of segments that switches
    /// between modes (such as alphanumeric and byte) to encode text in less space.
    /// This is a mid-level API; the high-level API is [`encode_text()`](QrCode::encode_text) and [`encode_binary()`](QrCode::encode_binary).
    ///
    /// Returns a wrapped `QrCode` if successful, or `Err` if the data is too
    /// long to fit in any version in the given range at the given ECC level.
    pub fn encode_segments_advanced(
        segments: &[QrSegment],
        mut ecl: ErrCorrectLvl,
        min_version: Version,
        max_version: Version,
        mask: Option<Mask>,
        boost_ecl: bool,
    ) -> Result<Self, DataTooLong> {
        assert!(min_version <= max_version, "Invalid value");

        // Find the minimal version number to use
        let mut version: Version = min_version;

        let data_used_bits: usize = loop {
            let data_capacity_bits: usize = QrCode::get_num_data_codewords(version, ecl) * 8; // Number of data bits available
            let data_used: Option<usize> = QrSegment::get_total_bits(segments, version);

            if data_used.is_some_and(|n| n <= data_capacity_bits) {
                break data_used.unwrap(); // This version number is found to be suitable
            } else if version >= max_version {
                // All versions in the range could not fit the given data
                return Err(match data_used {
                    None => DataTooLong::SegmentTooLong,
                    Some(n) => DataTooLong::DataOverCapacity(n, data_capacity_bits),
                });
            } else {
                version += 1;
            }
        };

        // Increase the error correction level while the data still fits in the current version number
        for &new_ecl in &[ErrCorrectLvl::Medium, ErrCorrectLvl::Quartile, ErrCorrectLvl::High] {
            // From low to high
            if boost_ecl && data_used_bits <= QrCode::get_num_data_codewords(version, new_ecl) * 8 {
                ecl = new_ecl;
            }
        }

        // Concatenate all segments to create the data bit string
        let mut bb = BitBuffer::from(Vec::new());
        for seg in segments {
            bb.append_bits(seg.mode.mode_bits(), 4);
            bb.append_bits(
                u32::try_from(seg.num_chars).unwrap(),
                seg.mode.num_char_count_bits(version),
            );
            bb.extend_from_slice(&seg.data);
        }
        debug_assert_eq!(bb.len(), data_used_bits);

        // Add terminator and pad up to a byte if applicable
        let data_capacity_bits: usize = QrCode::get_num_data_codewords(version, ecl) * 8;
        debug_assert!(bb.len() <= data_capacity_bits);
        let num_zero_bits: usize = std::cmp::min(4, data_capacity_bits - bb.len());
        bb.append_bits(0, u8::try_from(num_zero_bits).unwrap());
        let num_zero_bits: usize = bb.len().wrapping_neg() & 7;
        bb.append_bits(0, u8::try_from(num_zero_bits).unwrap());
        debug_assert_eq!(bb.len() % 8, 0);

        // Pad with alternating bytes until data capacity is reached
        for &pad_byte in [0xEC, 0x11].iter().cycle() {
            if bb.len() >= data_capacity_bits {
                break;
            }
            bb.append_bits(pad_byte, 8);
        }

        // Pack bits into bytes in big endian
        let mut data_codewords = vec![0u8; bb.len() / 8];
        for (i, &bit) in bb.iter().enumerate() {
            data_codewords[i >> 3] |= u8::from(bit) << (7 - (i & 7));
        }

        // Create the QR Code object
        Ok(QrCode::encode_codewords(version, ecl, &data_codewords, mask))
    }

    /*---- Constructor (low level) ----*/

    /// Creates a new QR Code with the given version number,
    /// error correction level, data codeword bytes, and mask number.
    ///
    /// This is a low-level API that most users should not use directly.
    /// A mid-level API is the [`encode_segments()`](QrCode::encode_segments) function.
    pub fn encode_codewords(ver: Version, ecl: ErrCorrectLvl, data_codewords: &[u8], mut msk: Option<Mask>) -> Self {
        // Initialize fields
        let size = usize::from(ver) * 4 + 17;
        let mut result = Self {
            version: ver,
            size: size as i32,
            mask: Mask::new(0), // Dummy value
            error_correction_level: ecl,
            modules: vec![false; size * size], // Initially all light
            is_function: vec![false; size * size],
        };

        // Compute ECC, draw modules
        result.draw_function_patterns();
        let all_codewords: Vec<u8> = result.add_ecc_and_interleave(data_codewords);
        result.draw_codewords(&all_codewords);

        // Do masking
        if msk.is_none() {
            // Automatically choose best mask
            let mut min_penalty = i32::MAX;
            for i in 0..8 {
                let i = Mask::new(i);
                result.apply_mask(i);
                result.draw_format_bits(i);
                let penalty: i32 = result.get_penalty_score();
                if penalty < min_penalty {
                    msk = Some(i);
                    min_penalty = penalty;
                }
                result.apply_mask(i); // Undoes the mask due to XOR
            }
        }
        let msk: Mask = msk.unwrap();
        result.mask = msk;
        result.apply_mask(msk); // Apply the final choice of mask
        result.draw_format_bits(msk); // Overwrite old format bits

        result.is_function.clear();
        result.is_function.shrink_to_fit();
        result
    }

    /*---- Public methods ----*/

    /// Returns this QR Code's version, in the range [1, 40].
    pub fn version(&self) -> Version {
        self.version
    }

    /// Returns this QR Code's size, in the range [21, 177].
    pub fn size(&self) -> i32 {
        self.size
    }

    /// Returns this QR Code's error correction level.
    pub fn error_correction_level(&self) -> ErrCorrectLvl {
        self.error_correction_level
    }

    /// Returns this QR Code's mask, in the range [0, 7].
    pub fn mask(&self) -> Mask {
        self.mask
    }

    /// Returns the color of the module (pixel) at the given coordinates,
    /// which is `false` for light or `true` for dark.
    ///
    /// The top left corner has the coordinates (x=0, y=0). If the given
    /// coordinates are out of bounds, then `false` (light) is returned.
    pub fn get_module(&self, x: i32, y: i32) -> bool {
        (0..self.size).contains(&x) && (0..self.size).contains(&y) && self.module(x, y)
    }

    /// Returns the color of the module at the given coordinates, which must be in bounds.
    fn module(&self, x: i32, y: i32) -> bool {
        self.modules[(y * self.size + x) as usize]
    }

    /// Returns a mutable reference to the module's color at the given coordinates, which must be in bounds.
    fn module_mut(&mut self, x: i32, y: i32) -> &mut bool {
        &mut self.modules[(y * self.size + x) as usize]
    }

    /*---- Private helper methods for constructor: Drawing function modules ----*/

    /// Reads this object's version field, and draws and marks all function modules.
    fn draw_function_patterns(&mut self) {
        // Draw horizontal and vertical timing patterns
        let size: i32 = self.size;
        for i in 0..size {
            self.set_function_module(6, i, i % 2 == 0);
            self.set_function_module(i, 6, i % 2 == 0);
        }

        // Draw 3 finder patterns (all corners except bottom right; overwrites some timing modules)
        self.draw_finder_pattern(3, 3);
        self.draw_finder_pattern(size - 4, 3);
        self.draw_finder_pattern(3, size - 4);

        // Draw numerous alignment patterns
        let align_pat_pos: Vec<i32> = self.get_alignment_pattern_positions();
        let num_align: usize = align_pat_pos.len();
        for i in 0..num_align {
            for j in 0..num_align {
                // Don't draw on the three finder corners
                if !(i == 0 && j == 0 || i == 0 && j == num_align - 1 || i == num_align - 1 && j == 0) {
                    self.draw_alignment_pattern(align_pat_pos[i], align_pat_pos[j]);
                }
            }
        }

        // Draw configuration data
        self.draw_format_bits(Mask::new(0)); // Dummy mask value; overwritten later in the constructor
        self.draw_version();
    }

    /// Draws two copies of the format bits (with its own error correction code)
    /// based on the given mask and this object's error correction level field.
    fn draw_format_bits(&mut self, mask: Mask) {
        // Calculate error correction code and pack bits
        let bits: u32 = {
            // errcorrlvl is uint2, mask is uint3
            let data = u32::from(self.error_correction_level.format_bits() << 3 | mask.value());
            let mut rem: u32 = data;
            for _ in 0..10 {
                rem = (rem << 1) ^ ((rem >> 9) * 0x537);
            }
            (data << 10 | rem) ^ 0x5412 // uint15
        };
        debug_assert_eq!(bits >> 15, 0);

        // Draw first copy
        for i in 0..6 {
            self.set_function_module(8, i, get_bit(bits, i));
        }
        self.set_function_module(8, 7, get_bit(bits, 6));
        self.set_function_module(8, 8, get_bit(bits, 7));
        self.set_function_module(7, 8, get_bit(bits, 8));
        for i in 9..15 {
            self.set_function_module(14 - i, 8, get_bit(bits, i));
        }

        // Draw second copy
        let size: i32 = self.size;
        for i in 0..8 {
            self.set_function_module(size - 1 - i, 8, get_bit(bits, i));
        }
        for i in 8..15 {
            self.set_function_module(8, size - 15 + i, get_bit(bits, i));
        }
        self.set_function_module(8, size - 8, true); // Always dark
    }

    /// Draws two copies of the version bits (with its own error correction code),
    /// based on this object's version field, iff 7 <= version <= 40.
    fn draw_version(&mut self) {
        if self.version < 7 {
            return;
        }

        // Calculate error correction code and pack bits
        let bits: u32 = {
            let data = u32::from(self.version); // uint6, in the range [7, 40]
            let mut rem: u32 = data;
            for _ in 0..12 {
                rem = (rem << 1) ^ ((rem >> 11) * 0x1F25);
            }
            data << 12 | rem // uint18
        };
        debug_assert_eq!(bits >> 18, 0);

        // Draw two copies
        for i in 0..18 {
            let bit: bool = get_bit(bits, i);
            let a: i32 = self.size - 11 + i % 3;
            let b: i32 = i / 3;
            self.set_function_module(a, b, bit);
            self.set_function_module(b, a, bit);
        }
    }

    /// Draws a `9*9` finder pattern including the border separator,
    /// with the center module at `(x, y)`. Modules can be out of bounds.
    fn draw_finder_pattern(&mut self, x: i32, y: i32) {
        for dy in -4..=4 {
            for dx in -4..=4 {
                let xx: i32 = x + dx;
                let yy: i32 = y + dy;
                if (0..self.size).contains(&xx) && (0..self.size).contains(&yy) {
                    let dist: i32 = std::cmp::max(dx.abs(), dy.abs()); // Chebyshev/infinity norm
                    self.set_function_module(xx, yy, dist != 2 && dist != 4);
                }
            }
        }
    }

    /// Draws a `5*5` alignment pattern, with the center module at `(x, y)`.
    /// All modules must be in bounds.
    fn draw_alignment_pattern(&mut self, x: i32, y: i32) {
        for dy in -2..=2 {
            for dx in -2..=2 {
                self.set_function_module(x + dx, y + dy, std::cmp::max(dx.abs(), dy.abs()) != 1);
            }
        }
    }

    /// Sets the color of a module and marks it as a function module.
    /// Only used by the constructor. Coordinates must be in bounds.
    fn set_function_module(&mut self, x: i32, y: i32, is_dark: bool) {
        *self.module_mut(x, y) = is_dark;
        self.is_function[(y * self.size + x) as usize] = true;
    }

    /*---- Private helper methods for constructor: Codewords and masking ----*/

    /// Returns a new byte string representing the given data with the appropriate error correction
    /// codewords appended to it, based on this object's version and error correction level.
    fn add_ecc_and_interleave(&self, data: &[u8]) -> Vec<u8> {
        let ver: Version = self.version;
        let ecl: ErrCorrectLvl = self.error_correction_level;
        assert_eq!(data.len(), QrCode::get_num_data_codewords(ver, ecl), "Illegal argument");

        // Calculate parameter numbers
        let num_blocks: usize = QrCode::table_get(&NUM_ERROR_CORRECTION_BLOCKS, ver, ecl);
        let block_ecc_len: usize = QrCode::table_get(&ECC_CODEWORDS_PER_BLOCK, ver, ecl);
        let raw_codewords: usize = QrCode::get_num_raw_data_modules(ver) / 8;
        let num_short_blocks: usize = num_blocks - raw_codewords % num_blocks;
        let short_block_len: usize = raw_codewords / num_blocks;

        // Split data into blocks and append ECC to each block
        let mut blocks = Vec::<Vec<u8>>::with_capacity(num_blocks);
        let rs_div: Vec<u8> = QrCode::reed_solomon_compute_divisor(block_ecc_len);
        let mut k: usize = 0;
        for i in 0..num_blocks {
            let dat_len: usize = short_block_len - block_ecc_len + usize::from(i >= num_short_blocks);
            let mut dat = data[k..k + dat_len].to_vec();
            k += dat_len;
            let ecc: Vec<u8> = QrCode::reed_solomon_compute_remainder(&dat, &rs_div);
            if i < num_short_blocks {
                dat.push(0);
            }
            dat.extend_from_slice(&ecc);
            blocks.push(dat);
        }

        // Interleave (not concatenate) the bytes from every block into a single sequence
        let mut result = Vec::<u8>::with_capacity(raw_codewords);
        for i in 0..=short_block_len {
            for (j, block) in blocks.iter().enumerate() {
                // Skip the padding byte in short blocks
                if i != short_block_len - block_ecc_len || j >= num_short_blocks {
                    result.push(block[i]);
                }
            }
        }
        result
    }

    /// Draws the given sequence of 8-bit codewords (data and error correction) onto the entire
    /// data area of this QR Code. Function modules need to be marked off before this is called.
    fn draw_codewords(&mut self, data: &[u8]) {
        assert_eq!(
            data.len(),
            QrCode::get_num_raw_data_modules(self.version) / 8,
            "Illegal argument"
        );

        let mut i: usize = 0; // Bit index into the data
        // Do the funny zigzag scan
        let mut right: i32 = self.size - 1;
        while right >= 1 {
            // Index of right column in each column pair
            if right == 6 {
                right = 5;
            }
            for vert in 0..self.size {
                // Vertical counter
                for j in 0..2 {
                    let x: i32 = right - j; // Actual x coordinate
                    let upward: bool = (right + 1) & 2 == 0;
                    let y: i32 = if upward { self.size - 1 - vert } else { vert }; // Actual y coordinate
                    if !self.is_function[(y * self.size + x) as usize] && i < data.len() * 8 {
                        *self.module_mut(x, y) = get_bit(u32::from(data[i >> 3]), 7 - ((i as i32) & 7));
                        i += 1;
                    }
                    // If this QR Code has any remainder bits (0 to 7), they were assigned as
                    // 0/false/light by the constructor and are left unchanged by this method
                }
            }
            right -= 2;
        }
        debug_assert_eq!(i, data.len() * 8);
    }

    /// XORs the codeword modules in this QR Code with the given mask pattern.
    ///
    /// The function modules must be marked and the codeword bits must be drawn
    /// before masking.
    ///
    /// Due to the arithmetic of XOR, calling [`apply_mask()`](QrCode::apply_mask) with
    /// the same mask value a second time will undo the mask. A final well-formed
    /// QR Code needs exactly one (not zero, two, etc.) mask applied.
    fn apply_mask(&mut self, mask: Mask) {
        for y in 0..self.size {
            for x in 0..self.size {
                let invert: bool = match mask.value() {
                    0 => (x + y) % 2 == 0,
                    1 => y % 2 == 0,
                    2 => x % 3 == 0,
                    3 => (x + y) % 3 == 0,
                    4 => (x / 3 + y / 2) % 2 == 0,
                    5 => x * y % 2 + x * y % 3 == 0,
                    6 => (x * y % 2 + x * y % 3) % 2 == 0,
                    7 => ((x + y) % 2 + x * y % 3) % 2 == 0,
                    _ => unreachable!(),
                };
                *self.module_mut(x, y) ^= invert & !self.is_function[(y * self.size + x) as usize];
            }
        }
    }

    /// Calculates and returns the penalty score based on state of this QR Code's current modules.
    /// This is used by the automatic mask choice algorithm to find the mask pattern that yields the lowest score.
    fn get_penalty_score(&self) -> i32 {
        const PENALTY_N1: i32 = 3;
        const PENALTY_N2: i32 = 3;
        const PENALTY_N3: i32 = 40;
        const PENALTY_N4: i32 = 10;

        let mut result: i32 = 0;
        let size: i32 = self.size;

        // Adjacent modules in row having same color, and finder-like patterns
        for y in 0..size {
            let mut run_color = false;
            let mut run_x: i32 = 0;
            let mut run_history = FinderPenalty::new(size);
            for x in 0..size {
                if self.module(x, y) == run_color {
                    run_x += 1;
                    if run_x == 5 {
                        result += PENALTY_N1;
                    } else if run_x > 5 {
                        result += 1;
                    }
                } else {
                    run_history.add_history(run_x);
                    if !run_color {
                        result += run_history.count_patterns() * PENALTY_N3;
                    }
                    run_color = self.module(x, y);
                    run_x = 1;
                }
            }
            result += run_history.terminate_and_count(run_color, run_x) * PENALTY_N3;
        }
        // Adjacent modules in column having same color, and finder-like patterns
        for x in 0..size {
            let mut run_color = false;
            let mut run_y: i32 = 0;
            let mut run_history = FinderPenalty::new(size);
            for y in 0..size {
                if self.module(x, y) == run_color {
                    run_y += 1;
                    if run_y == 5 {
                        result += PENALTY_N1;
                    } else if run_y > 5 {
                        result += 1;
                    }
                } else {
                    run_history.add_history(run_y);
                    if !run_color {
                        result += run_history.count_patterns() * PENALTY_N3;
                    }
                    run_color = self.module(x, y);
                    run_y = 1;
                }
            }
            result += run_history.terminate_and_count(run_color, run_y) * PENALTY_N3;
        }

        // 2*2 blocks of modules having same color
        for y in 0..size - 1 {
            for x in 0..size - 1 {
                let color: bool = self.module(x, y);
                if color == self.module(x + 1, y)
                    && color == self.module(x, y + 1)
                    && color == self.module(x + 1, y + 1)
                {
                    result += PENALTY_N2;
                }
            }
        }

        // Balance of dark and light modules
        let dark: i32 = self.modules.iter().copied().map(i32::from).sum();
        let total: i32 = size * size; // Note that size is odd, so dark/total != 1/2
        // Compute the smallest integer k >= 0 such that (45-5k)% <= dark/total <= (55+5k)%
        let k: i32 = ((dark * 20 - total * 10).abs() + total - 1) / total - 1;
        debug_assert!((0..=9).contains(&k));
        result += k * PENALTY_N4;
        debug_assert!((0..=2568888).contains(&result)); // Non-tight upper bound based on default values of PENALTY_N1, ..., N4
        result
    }

    /*---- Private helper functions ----*/

    /// Returns an ascending list of positions of alignment patterns for this version number.
    /// Each position is in the range `[0,177)`, and are used on both the x and y axes.
    /// This could be implemented as lookup table of 40 variable-length lists of unsigned bytes.
    fn get_alignment_pattern_positions(&self) -> Vec<i32> {
        let ver = i32::from(self.version);
        if ver == 1 {
            vec![]
        } else {
            let num_align: i32 = ver / 7 + 2;
            let step: i32 = (ver * 8 + num_align * 3 + 5) / (num_align * 4 - 4) * 2;
            let mut result: Vec<i32> = (0..num_align - 1).map(|i| self.size - 7 - i * step).collect();
            result.push(6);
            result.reverse();
            result
        }
    }

    /// Returns the number of data bits that can be stored in a QR Code of the given version number, after
    /// all function modules are excluded. This includes remainder bits, so it might not be a multiple of 8.
    /// The result is in the range `[208, 29648]`. This could be implemented as a 40-entry lookup table.
    fn get_num_raw_data_modules(version: Version) -> usize {
        let ver = usize::from(version);
        let mut result: usize = (16 * ver + 128) * ver + 64;
        if ver >= 2 {
            let num_align: usize = ver / 7 + 2;
            result -= (25 * num_align - 10) * num_align - 55;
            if ver >= 7 {
                result -= 36;
            }
        }
        debug_assert!((208..=29648).contains(&result));
        result
    }

    /// Returns the number of 8-bit data (i.e. not error correction) codewords contained in any
    /// QR Code of the given version number and error correction level, with remainder bits discarded.
    /// This stateless pure function could be implemented as a (40*4)-cell lookup table.
    pub(crate) fn get_num_data_codewords(ver: Version, ecl: ErrCorrectLvl) -> usize {
        QrCode::get_num_raw_data_modules(ver) / 8
            - QrCode::table_get(&ECC_CODEWORDS_PER_BLOCK, ver, ecl)
                * QrCode::table_get(&NUM_ERROR_CORRECTION_BLOCKS, ver, ecl)
    }

    /// Returns an entry from the given table based on the given values.
    fn table_get(table: &'static [[i8; 41]; 4], ver: Version, ecl: ErrCorrectLvl) -> usize {
        table[ecl.ordinal()][usize::from(ver)] as usize
    }

    /// Returns a Reed-Solomon ECC generator polynomial for the given degree. This could be
    /// implemented as a lookup table over all possible parameter values, instead of as an algorithm.
    fn reed_solomon_compute_divisor(degree: usize) -> Vec<u8> {
        assert!((1..=255).contains(&degree), "Degree out of range");
        // Polynomial coefficients are stored from highest to lowest power, excluding the leading term which is always 1.
        // For example the polynomial x^3 + 255x^2 + 8x + 93 is stored as the uint8 array [255, 8, 93].
        let mut result = vec![0u8; degree - 1];
        result.push(1); // Start off with the monomial x^0

        // Compute the product polynomial (x - r^0) * (x - r^1) * (x - r^2) * ... * (x - r^{degree-1}),
        // and drop the highest monomial term which is always 1x^degree.
        // Note that r = 0x02, which is a generator element of this field GF(2^8/0x11D).
        let mut root: u8 = 1;
        for _ in 0..degree {
            // Unused variable i
            // Multiply the current product by (x - r^i)
            for j in 0..degree {
                result[j] = QrCode::reed_solomon_multiply(result[j], root);
                if j + 1 < result.len() {
                    result[j] ^= result[j + 1];
                }
            }
            root = QrCode::reed_solomon_multiply(root, 0x02);
        }
        result
    }

    /// Returns the Reed-Solomon error correction codeword for the given data and divisor polynomials.
    fn reed_solomon_compute_remainder(data: &[u8], divisor: &[u8]) -> Vec<u8> {
        let mut result = vec![0u8; divisor.len()];
        for b in data {
            // Polynomial division
            let factor: u8 = b ^ result.remove(0);
            result.push(0);
            for (x, &y) in result.iter_mut().zip(divisor.iter()) {
                *x ^= QrCode::reed_solomon_multiply(y, factor);
            }
        }
        result
    }

    /// Returns the product of the two given field elements modulo GF(2^8/0x11D).
    /// All inputs are valid. This could be implemented as a 256*256 lookup table.
    fn reed_solomon_multiply(x: u8, y: u8) -> u8 {
        // Russian peasant multiplication
        let mut z: u8 = 0;
        for i in (0..8).rev() {
            z = (z << 1) ^ ((z >> 7) * 0x1D);
            z ^= ((y >> i) & 1) * x;
        }
        z
    }
}

#[rustfmt::skip]
static ECC_CODEWORDS_PER_BLOCK: [[i8; 41]; 4] = [
    // Version: (index 0 is for padding, and is set to an illegal value)
    //0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40    Error correction level
    [-1, 7,  10, 15, 20, 26, 18, 20, 24, 30, 18, 20, 24, 26, 30, 22, 24, 28, 30, 28, 28, 28, 28, 30, 30, 26, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30],  // Low
    [-1, 10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28], // Medium
    [-1, 13, 22, 18, 26, 18, 24, 18, 22, 20, 24, 28, 26, 24, 20, 30, 24, 28, 28, 26, 30, 28, 30, 30, 30, 30, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30], // Quartile
    [-1, 17, 28, 22, 16, 22, 28, 26, 26, 24, 28, 24, 28, 22, 24, 24, 30, 28, 28, 26, 28, 30, 24, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30], // High
];

#[rustfmt::skip]
static NUM_ERROR_CORRECTION_BLOCKS: [[i8; 41]; 4] = [
    // Version: (index 0 is for padding, and is set to an illegal value)
    //0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40    Error correction level
    [-1, 1, 1, 1, 1, 1, 2, 2, 2, 2,  4,  4,  4,  4,  4,  6,  6,  6,  6,  7,  8,  8,  9,  9, 10, 12, 12, 12, 13, 14, 15, 16, 17, 18, 19, 19, 20, 21, 22, 24, 25], // Low
    [-1, 1, 1, 1, 2, 2, 4, 4, 4, 5,  5,  5,  8,  9,  9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21, 23, 25, 26, 28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49], // Medium
    [-1, 1, 1, 2, 2, 4, 4, 6, 6, 8,  8,  8, 10, 12, 16, 12, 17, 16, 18, 21, 20, 23, 23, 25, 27, 29, 34, 34, 35, 38, 40, 43, 45, 48, 51, 53, 56, 59, 62, 65, 68], // Quartile
    [-1, 1, 1, 2, 4, 4, 4, 5, 6, 8,  8, 11, 11, 16, 16, 18, 16, 19, 21, 25, 25, 25, 34, 30, 32, 35, 37, 40, 42, 45, 48, 51, 54, 57, 60, 63, 66, 70, 74, 77, 81], // High
];
