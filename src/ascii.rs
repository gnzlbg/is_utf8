//! ASCII utilities

/// Returns `Ok` if the byte-slice is ascii, and the largest index for which
/// `is_ascii(&[..index])` returns `Ok`, that is, the index of the first
/// non-ASCII byte.
pub fn is_ascii_scalar(x: &[u8]) -> Result<(), usize> {
    for (i,b) in x.iter().enumerate() {
        if !b.is_ascii() { return Err(i); }
    }
    Ok(())
}

pub fn is_ascii_vector128(s: &[u8]) -> Result<(), usize> {
    use ::simd::*;
    let mut i = 0;
    let v128 = u8x16::splat(128);
    let zero = u8x16::splat(0);
    let len = s.len();
    while i + u8x16::lanes() * 2 <= len {
        let x = unsafe { u8x16::load_unaligned_unchecked(&s.get_unchecked(i..)) };
        let y = unsafe { u8x16::load_unaligned_unchecked(&s.get_unchecked(i + u8x16::lanes()..)) };
        let x: u8x16 = x & v128;
        let y: u8x16 = y & v128;
        if !x.eq(zero).all() || !y.eq(zero).all() {
            break;
        }
        i += u8x16::lanes() * 2;
    }
    is_ascii_scalar(unsafe { &s.get_unchecked(i..) }).map_err(|e| e + i)
}

/// _mm_testz_si128 requires SSE4.1
#[target_feature(enable = "sse4.1")]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub unsafe fn is_ascii_vector128_sse41(x: &[u8]) -> Result<(), usize> {
    use ::arch::*;
    let mut i = 0;
    let signbitmask = _mm_set1_epi8(::mem::transmute(0b1000_0000_u8));
    let ptr = x.as_ptr();
    let len = x.len();
    while i + 64 <= len {
        let x0 = _mm_loadu_si128(ptr.offset(i as isize) as *const __m128i);
        let x1 = _mm_loadu_si128(ptr.offset(i as isize + 16) as *const __m128i);
        let x2 = _mm_loadu_si128(ptr.offset(i as isize + 32) as *const __m128i);
        let x3 = _mm_loadu_si128(ptr.offset(i as isize + 48) as *const __m128i);
        if _mm_testz_si128(x0, signbitmask) == 0
            || _mm_testz_si128(x1, signbitmask) == 0
            || _mm_testz_si128(x2, signbitmask) == 0
            || _mm_testz_si128(x3, signbitmask) == 0 {
            break;
        }
        i += 64;
    }
    is_ascii_scalar(&x[i..]).map_err(|e| e + i)
}

#[target_feature(enable = "avx")]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub unsafe fn is_ascii_vector256_avx(x: &[u8]) -> Result<(), usize> {
    use ::arch::*;
    let mut i = 0;
    let signbitmask = _mm256_set1_epi8(::mem::transmute(0b1000_0000_u8));
    let ptr = x.as_ptr();
    let len = x.len();
    while i + 128 <= len {
        let x0 = _mm256_loadu_si256(ptr.offset(i as isize) as *const __m256i);
        let x1 = _mm256_loadu_si256(ptr.offset(i as isize + 32) as *const __m256i);
        let x2 = _mm256_loadu_si256(ptr.offset(i as isize + 64) as *const __m256i);
        let x3 = _mm256_loadu_si256(ptr.offset(i as isize + 96) as *const __m256i);
        if _mm256_testz_si256(x0, signbitmask) == 0
            || _mm256_testz_si256(x1, signbitmask) == 0
            || _mm256_testz_si256(x2, signbitmask) == 0
            || _mm256_testz_si256(x3, signbitmask) == 0 {
            break;
        }
        i += 128;
    }
    is_ascii_scalar(&x[i..]).map_err(|e| e + i)
}


#[cfg(test)]
mod tests {
    use super::*;

    fn test_is_slice_ascii<F>(f: F)
        where F: Fn(&[u8]) -> Result<(), usize>
    {
        for i in 0..=127 {
            for j in i..=127 {
                let v = (i..j).collect::<Vec<u8>>();
                assert!(f(v.as_slice()).is_ok());
            }
            for j in 128..=u8::max_value() {
                let v = (i..=j).collect::<Vec<u8>>();
                let r = f(v.as_slice());
                assert!(r.is_err());
                assert_eq!(r.unwrap_err(), 128 - i as usize);
            }
        }
        for i in 128..=u8::max_value() {
            for j in i..=u8::max_value() {
                let v = (i..=j).collect::<Vec<u8>>();
                let r = f(v.as_slice());
                assert!(r.is_err());
                assert_eq!(r.unwrap_err(), 0 as usize);
            }
        }
    }


    #[test]
    fn test_is_ascii_scalar() {
        test_is_slice_ascii(is_ascii_scalar);
    }
    #[test]
    fn test_is_ascii_vector128() {
        test_is_slice_ascii(is_ascii_vector128);
    }
    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse4.1"))]
    #[test]
    fn test_is_ascii_vector128_sse41() {
        test_is_slice_ascii(|x| unsafe { is_ascii_vector128_sse41(x) });
    }

    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "avx"))]
    #[test]
    fn test_is_ascii_vector256_avx() {
        test_is_slice_ascii(|x| unsafe { is_ascii_vector256_avx(x) });
    }
}
