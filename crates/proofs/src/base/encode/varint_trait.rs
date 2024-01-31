/**
 * Adapted from integer-encoding-rs
 *
 * See third_party/license/integer-encoding.LICENSE
 */
// ---------------------------------------------------------------------------------------------------------------
// The following chunck of code is copied from the `integer-encoding`. This is for two reasons:
// 1) it makes the `VarInt` no longer a foreign trait
// 2) there is a bug in `integer-encoding` that made it so that large decodings didn't fail when they should have
// There were significant code changes to simplify the code
// ---------------------------------------------------------------------------------------------------------------

/// Most-significant byte, == 0x80
pub const MSB: u8 = 0b1000_0000;
/// All bits except for the most significant. Can be used as bitmask to drop the most-signficant
/// bit using `&` (binary-and).
const DROP_MSB: u8 = 0b0111_1111;

/// Varint (variable length integer) encoding, as described in
/// https://developers.google.com/protocol-buffers/docs/encoding.
///
/// Uses zigzag encoding (also described there) for signed integer representation.
pub trait VarInt: Sized + Copy {
    /// Returns the number of bytes this number needs in its encoded form. Note: This varies
    /// depending on the actual number you want to encode.
    fn required_space(self) -> usize;
    /// Decode a value from the slice. Returns the value and the number of bytes read from the
    /// slice (can be used to read several consecutive values from a big slice)
    /// return None if the decoded value overflows this type.
    fn decode_var(src: &[u8]) -> Option<(Self, usize)>;
    /// Encode a value into the slice. The slice must be at least `required_space()` bytes long.
    /// The number of bytes taken by the encoded integer is returned.
    fn encode_var(self, src: &mut [u8]) -> usize;

    /// Helper: Encode a value and return the encoded form as Vec. The Vec must be at least
    /// `required_space()` bytes long.
    #[cfg(test)]
    fn encode_var_vec(self) -> Vec<u8> {
        let mut v = vec![0; self.required_space()];
        self.encode_var(&mut v);
        v
    }
}

#[inline]
fn zigzag_encode(from: i64) -> u64 {
    ((from << 1) ^ (from >> 63)) as u64
}

// see: http://stackoverflow.com/a/2211086/56332
// casting required because operations like unary negation
// cannot be performed on unsigned integers
#[inline]
fn zigzag_decode(from: u64) -> i64 {
    ((from >> 1) ^ (-((from & 1) as i64)) as u64) as i64
}

macro_rules! impl_varint {
    ($t:ty, unsigned) => {
        impl VarInt for $t {
            fn required_space(self) -> usize {
                (self as u64).required_space()
            }

            fn decode_var(src: &[u8]) -> Option<(Self, usize)> {
                let (n, s) = u64::decode_var(src)?;
                // This check is required to ensure that we actually return `None` when `src` has a value that would overflow `Self`.
                if n > (Self::MAX as u64) {
                    None
                } else {
                    Some((n as Self, s))
                }
            }

            fn encode_var(self, dst: &mut [u8]) -> usize {
                (self as u64).encode_var(dst)
            }
        }
    };
    ($t:ty, signed) => {
        impl VarInt for $t {
            fn required_space(self) -> usize {
                (self as i64).required_space()
            }

            fn decode_var(src: &[u8]) -> Option<(Self, usize)> {
                let (n, s) = i64::decode_var(src)?;
                // This check is required to ensure that we actually return `None` when `src` has a value that would overflow `Self`.
                if n > (Self::MAX as i64) || n < (Self::MIN as i64) {
                    None
                } else {
                    Some((n as Self, s))
                }
            }

            fn encode_var(self, dst: &mut [u8]) -> usize {
                (self as i64).encode_var(dst)
            }
        }
    };
}

impl_varint!(usize, unsigned);
impl_varint!(u32, unsigned);
impl_varint!(u16, unsigned);
impl_varint!(u8, unsigned);

impl_varint!(isize, signed);
impl_varint!(i32, signed);
impl_varint!(i16, signed);
impl_varint!(i8, signed);

// Below are the "base implementations" doing the actual encodings; all other integer types are
// first cast to these biggest types before being encoded.

impl VarInt for u64 {
    fn required_space(self) -> usize {
        let bits = 64 - self.leading_zeros() as usize;
        core::cmp::max(1, (bits + 6) / 7)
    }

    #[inline]
    fn decode_var(src: &[u8]) -> Option<(Self, usize)> {
        let mut result: u64 = 0;
        let mut shift = 0;

        let mut success = false;
        for b in src.iter() {
            let msb_dropped = b & DROP_MSB;
            result |= (msb_dropped as u64) << shift;
            shift += 7;

            if shift > (9 * 7) {
                // This check is required to ensure that we actually return `None` when `src` has a value that would overflow `u64`.
                success = *b < 2;
                break;
            } else if b & MSB == 0 {
                success = true;
                break;
            }
        }

        if success {
            Some((result, shift / 7))
        } else {
            None
        }
    }

    #[inline]
    fn encode_var(self, dst: &mut [u8]) -> usize {
        assert!(dst.len() >= self.required_space());
        let mut n = self;
        let mut i = 0;

        while n >= 0x80 {
            dst[i] = MSB | (n as u8);
            i += 1;
            n >>= 7;
        }

        dst[i] = n as u8;
        i + 1
    }
}

impl VarInt for i64 {
    fn required_space(self) -> usize {
        zigzag_encode(self).required_space()
    }

    #[inline]
    fn decode_var(src: &[u8]) -> Option<(Self, usize)> {
        let (result, size) = u64::decode_var(src)?;
        Some((zigzag_decode(result), size))
    }

    #[inline]
    fn encode_var(self, dst: &mut [u8]) -> usize {
        zigzag_encode(self).encode_var(dst)
    }
}