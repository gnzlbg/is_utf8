//! Is a byte slice `&[u8]` a valid UTF8 string.

// Copyright 2012-2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(align_offset)]
#![no_std]

use core::mem;
use core::str::Utf8Error;

/// https://tools.ietf.org/html/rfc3629
#[cfg_attr(rustfmt, rustfmt_skip)]
static UTF8_CHAR_WIDTH: [u8; 256] = [
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, // 0x1F
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, // 0x3F
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, // 0x5F
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, // 0x7F
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0, // 0x9F
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0, // 0xBF
    0,0,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
    2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2, // 0xDF
    3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3, // 0xEF
    4,4,4,4,4,0,0,0,0,0,0,0,0,0,0,0, // 0xFF
];

/// Mask of the value bits of a continuation byte.
const CONT_MASK: u8 = 0b0011_1111;
/// Value of the tag bits (tag mask is !CONT_MASK) of a continuation byte.
const TAG_CONT_U8: u8 = 0b1000_0000;

// use truncation to fit u64 into usize
const NONASCII_MASK: usize = 0x80808080_80808080u64 as usize;

/// Returns `true` if any byte in the word `x` is nonascii (>= 128).
#[inline]
fn contains_nonascii(x: usize) -> bool {
    (x & NONASCII_MASK) != 0
}

/// Workaround the internals of Utf8Error
struct Utf8ErrorImpl(usize, Option<u8>);

impl Utf8ErrorImpl {
    fn get(self) -> Utf8Error {
        unsafe { mem::transmute(self) }
    }
}

#[inline]
pub fn is_utf8(v: &[u8]) -> Result<(), Utf8Error> {
    let mut index = 0;
    let len = v.len();

    let usize_bytes = mem::size_of::<usize>();
    let ascii_block_size = 2 * usize_bytes;
    let blocks_end = if len >= ascii_block_size {
        len - ascii_block_size + 1
    } else {
        0
    };

    while index < len {
        let old_offset = index;
        macro_rules! err {
            ($error_len:expr) => {
                return Err(Utf8ErrorImpl(old_offset, $error_len).get());
            };
        }

        macro_rules! next {
            () => {{
                index += 1;
                // we needed data, but there was none: error!
                if index >= len {
                    err!(None)
                }
                v[index]
            }};
        }

        let first = v[index];
        if first >= 128 {
            let w = UTF8_CHAR_WIDTH[first as usize];
            // 2-byte encoding is for codepoints  \u{0080} to  \u{07ff}
            //        first  C2 80        last DF BF
            // 3-byte encoding is for codepoints  \u{0800} to  \u{ffff}
            //        first  E0 A0 80     last EF BF BF
            //   excluding surrogates codepoints  \u{d800} to  \u{dfff}
            //               ED A0 80 to       ED BF BF
            // 4-byte encoding is for codepoints \u{1000}0 to \u{10ff}ff
            //        first  F0 90 80 80  last F4 8F BF BF
            //
            // Use the UTF-8 syntax from the RFC
            //
            // https://tools.ietf.org/html/rfc3629
            // UTF8-1      = %x00-7F
            // UTF8-2      = %xC2-DF UTF8-tail
            // UTF8-3      = %xE0 %xA0-BF UTF8-tail / %xE1-EC 2( UTF8-tail ) /
            //               %xED %x80-9F UTF8-tail / %xEE-EF 2( UTF8-tail )
            // UTF8-4      = %xF0 %x90-BF 2( UTF8-tail ) / %xF1-F3 3( UTF8-tail ) /
            //               %xF4 %x80-8F 2( UTF8-tail )
            match w {
                2 => if next!() & !CONT_MASK != TAG_CONT_U8 {
                    err!(Some(1))
                },
                3 => {
                    match (first, next!()) {
                        (0xE0, 0xA0...0xBF)
                        | (0xE1...0xEC, 0x80...0xBF)
                        | (0xED, 0x80...0x9F)
                        | (0xEE...0xEF, 0x80...0xBF) => {}
                        _ => err!(Some(1)),
                    }
                    if next!() & !CONT_MASK != TAG_CONT_U8 {
                        err!(Some(2))
                    }
                }
                4 => {
                    match (first, next!()) {
                        (0xF0, 0x90...0xBF)
                        | (0xF1...0xF3, 0x80...0xBF)
                        | (0xF4, 0x80...0x8F) => {}
                        _ => err!(Some(1)),
                    }
                    if next!() & !CONT_MASK != TAG_CONT_U8 {
                        err!(Some(2))
                    }
                    if next!() & !CONT_MASK != TAG_CONT_U8 {
                        err!(Some(3))
                    }
                }
                _ => err!(Some(1)),
            }
            index += 1;
        } else {
            // Ascii case, try to skip forward quickly.
            // When the pointer is aligned, read 2 words of data per iteration
            // until we find a word containing a non-ascii byte.
            let ptr = v.as_ptr();
            let align = unsafe {
                // the offset is safe, because `index` is guaranteed inbounds
                ptr.offset(index as isize).align_offset(usize_bytes)
            };
            if align == 0 {
                while index < blocks_end {
                    unsafe {
                        let block = ptr.offset(index as isize) as *const usize;
                        // break if there is a nonascii byte
                        let zu = contains_nonascii(*block);
                        let zv = contains_nonascii(*block.offset(1));
                        if zu | zv {
                            break;
                        }
                    }
                    index += ascii_block_size;
                }
                // step from the point where the wordwise loop stopped
                while index < len && v[index] < 128 {
                    index += 1;
                }
            } else {
                index += 1;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_utf8;

    #[test]
    fn test_is_utf8() {
        // deny overlong encodings
        assert!(is_utf8(&[0xc0, 0x80]).is_err());
        assert!(is_utf8(&[0xc0, 0xae]).is_err());
        assert!(is_utf8(&[0xe0, 0x80, 0x80]).is_err());
        assert!(is_utf8(&[0xe0, 0x80, 0xaf]).is_err());
        assert!(is_utf8(&[0xe0, 0x81, 0x81]).is_err());
        assert!(is_utf8(&[0xf0, 0x82, 0x82, 0xac]).is_err());
        assert!(is_utf8(&[0xf4, 0x90, 0x80, 0x80]).is_err());

        // deny surrogates
        assert!(is_utf8(&[0xED, 0xA0, 0x80]).is_err());
        assert!(is_utf8(&[0xED, 0xBF, 0xBF]).is_err());

        assert!(is_utf8(&[0xC2, 0x80]).is_ok());
        assert!(is_utf8(&[0xDF, 0xBF]).is_ok());
        assert!(is_utf8(&[0xE0, 0xA0, 0x80]).is_ok());
        assert!(is_utf8(&[0xED, 0x9F, 0xBF]).is_ok());
        assert!(is_utf8(&[0xEE, 0x80, 0x80]).is_ok());
        assert!(is_utf8(&[0xEF, 0xBF, 0xBF]).is_ok());
        assert!(is_utf8(&[0xF0, 0x90, 0x80, 0x80]).is_ok());
        assert!(is_utf8(&[0xF4, 0x8F, 0xBF, 0xBF]).is_ok());

        // from: http://www.cl.cam.ac.uk/~mgk25/ucs/examples/UTF-8-test.txt
        assert!(is_utf8("κόσμε".as_bytes()).is_ok());

        // 2.1 First possible sequence of a certain length: 1 to 6 bytes
        assert!(is_utf8(&[0]).is_ok());
        assert!(is_utf8(&[0xC2, 0x80]).is_ok());
        assert!(is_utf8(&[0xE0, 0xA0, 0x80]).is_ok());
        assert!(is_utf8(&[0xF0, 0x90, 0x80, 0x80]).is_ok());
        assert!(is_utf8(&[0xF8, 0x88, 0x80, 0x80, 0x80]).is_err());
        assert!(is_utf8(&[0xFC, 0x84, 0x80, 0x80, 0x80, 0x80]).is_err());

        // 2.2 Last possible sequence of a certain length: 1 to 6 bytes
        assert!(is_utf8(&[0x7F]).is_ok());
        assert!(is_utf8(&[0xDF, 0xBF]).is_ok());
        assert!(is_utf8(&[0xEF, 0xBF, 0xBF]).is_ok());
        assert!(is_utf8(&[0xF7, 0xBF, 0xBF, 0xBF]).is_err());
        assert!(is_utf8(&[0xFB, 0xBF, 0xBF, 0xBF, 0xBF]).is_err());
        assert!(is_utf8(&[0xFD, 0xBF, 0xBF, 0xBF, 0xBF, 0xBF]).is_err());

        // 2.3 Other boundary conditions
        assert!(is_utf8(&[0xED, 0x9F, 0xBF]).is_ok());
        assert!(is_utf8(&[0xEE, 0x80, 0x80]).is_ok());
        assert!(is_utf8(&[0xEF, 0xBF, 0xBD]).is_ok());
        assert!(is_utf8(&[0xF4, 0x8F, 0xBF, 0xBF]).is_ok());
        assert!(is_utf8(&[0xF4, 0x90, 0x80, 0x80]).is_err());

        // 3.1  Unexpected continuation bytes
        assert!(is_utf8(&[0x80]).is_err());
        assert!(is_utf8(&[0xbf]).is_err());
        assert!(is_utf8(&[0x80, 0xBF]).is_err());
        assert!(is_utf8(&[0x80, 0xBF, 0x80]).is_err());
        assert!(is_utf8(&[0x80, 0xBF, 0x80, 0xBF]).is_err());
        assert!(is_utf8(&[0x80, 0xBF, 0x80, 0xBF, 0x80]).is_err());
        assert!(is_utf8(&[0x80, 0xBF, 0x80, 0xBF, 0x80, 0xBF]).is_err());
        assert!(is_utf8(&[0x80, 0xBF, 0x80, 0xBF, 0x80, 0xBF, 0x80]).is_err());

        // 3.1.9 Sequence of all 64 possible continuation bytes (0x80-0xbf):
        #[cfg_attr(rustfmt, rustfmt_skip)]
        let continuation_bytes = [
            0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
            0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E, 0x8F,
            0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97,
            0x98, 0x99, 0x9A, 0x9B, 0x9C, 0x9D, 0x9E, 0x9F,
            0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7,
            0xA8, 0xA9, 0xAA, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF,
            0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7,
            0xB8, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF,
        ];
        assert!(is_utf8(&continuation_bytes).is_err());
        for &b in continuation_bytes.iter() {
            assert!(is_utf8(&[b]).is_err());
        }

        // 3.2  Lonely start characters
        #[cfg_attr(rustfmt, rustfmt_skip)]
        let lonely_start_characters_2 = [
            0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7,
            0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0xCE, 0xCF,
            0xD0, 0xD1, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7,
            0xD8, 0xD9, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF,
        ];
        assert!(is_utf8(&lonely_start_characters_2).is_err());
        for &b in &lonely_start_characters_2 {
            assert!(is_utf8(&[b]).is_err());
        }

        #[cfg_attr(rustfmt, rustfmt_skip)]
        let lonely_start_characters_3 = [
            0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7,
            0xE8, 0xE9, 0xEA, 0xEB, 0xEC, 0xED, 0xEE, 0xEF,
        ];
        assert!(is_utf8(&lonely_start_characters_3).is_err());
        for &b in &lonely_start_characters_3 {
            assert!(is_utf8(&[b]).is_err());
        }

        #[cfg_attr(rustfmt, rustfmt_skip)]
        let lonely_start_characters_4 = [
            0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7,
        ];
        assert!(is_utf8(&lonely_start_characters_4).is_err());
        for &b in &lonely_start_characters_4 {
            assert!(is_utf8(&[b]).is_err());
        }

        let lonely_start_characters_5 = [0xF8, 0xF9, 0xFA, 0xFB];
        assert!(is_utf8(&lonely_start_characters_5).is_err());
        for &b in &lonely_start_characters_5 {
            assert!(is_utf8(&[b]).is_err());
        }

        let lonely_start_characters_6 = [0xFC, 0xFD];
        assert!(is_utf8(&lonely_start_characters_6).is_err());
        for &b in &lonely_start_characters_6 {
            assert!(is_utf8(&[b]).is_err());
        }

        // 3.3 Sequences with last continuation byte missing
        assert!(is_utf8(&[0xC0]).is_err());
        assert!(is_utf8(&[0xE0, 0x80]).is_err());
        assert!(is_utf8(&[0xF0, 0x80, 0x80]).is_err());
        assert!(is_utf8(&[0xF8, 0x80, 0x80, 0x80]).is_err());
        assert!(is_utf8(&[0xFC, 0x80, 0x80, 0x80, 0x80]).is_err());
        assert!(is_utf8(&[0xDF]).is_err());
        assert!(is_utf8(&[0xEF, 0xBF]).is_err());
        assert!(is_utf8(&[0xF7, 0xBF, 0xBF]).is_err());
        assert!(is_utf8(&[0xFB, 0xBF, 0xBF, 0xBF]).is_err());
        assert!(is_utf8(&[0xFD, 0xBF, 0xBF, 0xBF, 0xBF]).is_err());

        // 3.4 Concatenation of incomplete sequences
        #[cfg_attr(rustfmt, rustfmt_skip)]
        let incomplete = [
            0xC0,
            0xE0, 0x80,
            0xF0, 0x80, 0x80,
            0xF8, 0x80, 0x80, 0x80,
            0xFC, 0x80, 0x80, 0x80, 0x80,
            0xDF,
            0xEF, 0xBF,
            0xF7, 0xBF, 0xBF,
            0xFB, 0xBF, 0xBF, 0xBF,
            0xFD, 0xBF, 0xBF, 0xBF, 0xBF];
        assert!(is_utf8(&incomplete).is_err());

        // 3.5 Impossible bytes
        assert!(is_utf8(&[0xFE]).is_err());
        assert!(is_utf8(&[0xFF]).is_err());
        assert!(is_utf8(&[0xFE, 0xFE, 0xFF, 0xFF]).is_err());

        // 4. Overlong sequences
        assert!(is_utf8(&[0xC0, 0xAF]).is_err());
        assert!(is_utf8(&[0xE0, 0x80, 0xAF]).is_err());
        assert!(is_utf8(&[0xF0, 0x80, 0x80, 0xAF]).is_err());
        assert!(is_utf8(&[0xF8, 0x80, 0x80, 0x80, 0xAF]).is_err());
        assert!(is_utf8(&[0xFC, 0x80, 0x80, 0x80, 0x80, 0xAF]).is_err());

        assert!(is_utf8(&[0xC0, 0x80]).is_err());
        assert!(is_utf8(&[0xE0, 0x80, 0x80]).is_err());
        assert!(is_utf8(&[0xF0, 0x80, 0x80, 0x80]).is_err());
        assert!(is_utf8(&[0xF8, 0x80, 0x80, 0x80, 0x80]).is_err());
        assert!(is_utf8(&[0xFC, 0x80, 0x80, 0x80, 0x80, 0x80]).is_err());

        // 5. Illegal code positions
        assert!(is_utf8(&[0xed, 0xa0, 0x80]).is_err());
        assert!(is_utf8(&[0xed, 0xad, 0xbf]).is_err());
        assert!(is_utf8(&[0xed, 0xae, 0x80]).is_err());
        assert!(is_utf8(&[0xed, 0xaf, 0xbf]).is_err());
        assert!(is_utf8(&[0xed, 0xb0, 0x80]).is_err());
        assert!(is_utf8(&[0xed, 0xbe, 0x80]).is_err());
        assert!(is_utf8(&[0xed, 0xbf, 0xbf]).is_err());

        assert!(is_utf8(&[0xed, 0xa0, 0x80, 0xed, 0xb0, 0x80]).is_err());
        assert!(is_utf8(&[0xed, 0xa0, 0x80, 0xed, 0xbf, 0xbf]).is_err());
        assert!(is_utf8(&[0xed, 0xad, 0xbf, 0xed, 0xb0, 0x80]).is_err());
        assert!(is_utf8(&[0xed, 0xad, 0xbf, 0xed, 0xbf, 0xbf]).is_err());
        assert!(is_utf8(&[0xed, 0xae, 0x80, 0xed, 0xb0, 0x80]).is_err());
        assert!(is_utf8(&[0xed, 0xae, 0x80, 0xed, 0xbf, 0xbf]).is_err());
        assert!(is_utf8(&[0xed, 0xaf, 0xbf, 0xed, 0xb0, 0x80]).is_err());
        assert!(is_utf8(&[0xed, 0xaf, 0xbf, 0xed, 0xbf, 0xbf]).is_err());
    }

    const UTF8_SAMPLE_OK: &str = r#"
UTF-8 encoded sample plain-text file
‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾

Markus Kuhn [ˈmaʳkʊs kuːn] &lt;mkuhn@acm.org> — 1999-08-20


The ASCII compatible UTF-8 encoding of ISO 10646 and Unicode
plain-text files is defined in RFC 2279 and in ISO 10646-1 Annex R.


Using Unicode/UTF-8, you can write in emails and source code things such as

Mathematics and Sciences:

  ∮ E⋅da = Q,  n → ∞, ∑ f(i) = ∏ g(i), ∀x∈ℝ: ⌈x⌉ = −⌊−x⌋, α ∧ ¬β = ¬(¬α ∨ β),

  ℕ ⊆ ℕ₀ ⊂ ℤ ⊂ ℚ ⊂ ℝ ⊂ ℂ, ⊥ &lt; a ≠ b ≡ c ≤ d ≪ ⊤ ⇒ (A ⇔ B),

  2H₂ + O₂ ⇌ 2H₂O, R = 4.7 kΩ, ⌀ 200 mm

Linguistics and dictionaries:

  ði ıntəˈnæʃənəl fəˈnɛtık əsoʊsiˈeıʃn
  Y [ˈʏpsilɔn], Yen [jɛn], Yoga [ˈjoːgɑ]

APL:

  ((V⍳V)=⍳⍴V)/V←,V    ⌷←⍳→⍴∆∇⊃‾⍎⍕⌈

Nicer typography in plain text files:

  ╔══════════════════════════════════════════╗
  ║                                          ║
  ║   • ‘single’ and “double” quotes         ║
  ║                                          ║
  ║   • Curly apostrophes: “We’ve been here” ║
  ║                                          ║
  ║   • Latin-1 apostrophe and accents: '´`  ║
  ║                                          ║
  ║   • ‚deutsche‘ „Anführungszeichen“       ║
  ║                                          ║
  ║   • †, ‡, ‰, •, 3–4, —, −5/+5, ™, …      ║
  ║                                          ║
  ║   • ASCII safety test: 1lI|, 0OD, 8B     ║
  ║                      ╭─────────╮         ║
  ║   • the euro symbol: │ 14.95 € │         ║
  ║                      ╰─────────╯         ║
  ╚══════════════════════════════════════════╝

Greek (in Polytonic):

  The Greek anthem:

  Σὲ γνωρίζω ἀπὸ τὴν κόψη
  τοῦ σπαθιοῦ τὴν τρομερή,
  σὲ γνωρίζω ἀπὸ τὴν ὄψη
  ποὺ μὲ βία μετράει τὴ γῆ.

  ᾿Απ᾿ τὰ κόκκαλα βγαλμένη
  τῶν ῾Ελλήνων τὰ ἱερά
  καὶ σὰν πρῶτα ἀνδρειωμένη
  χαῖρε, ὦ χαῖρε, ᾿Ελευθεριά!

  From a speech of Demosthenes in the 4th century BC:

  Οὐχὶ ταὐτὰ παρίσταταί μοι γιγνώσκειν, ὦ ἄνδρες ᾿Αθηναῖοι,
  ὅταν τ᾿ εἰς τὰ πράγματα ἀποβλέψω καὶ ὅταν πρὸς τοὺς
  λόγους οὓς ἀκούω· τοὺς μὲν γὰρ λόγους περὶ τοῦ
  τιμωρήσασθαι Φίλιππον ὁρῶ γιγνομένους, τὰ δὲ πράγματ᾿ 
  εἰς τοῦτο προήκοντα,  ὥσθ᾿ ὅπως μὴ πεισόμεθ᾿ αὐτοὶ
  πρότερον κακῶς σκέψασθαι δέον. οὐδέν οὖν ἄλλο μοι δοκοῦσιν
  οἱ τὰ τοιαῦτα λέγοντες ἢ τὴν ὑπόθεσιν, περὶ ἧς βουλεύεσθαι,
  οὐχὶ τὴν οὖσαν παριστάντες ὑμῖν ἁμαρτάνειν. ἐγὼ δέ, ὅτι μέν
  ποτ᾿ ἐξῆν τῇ πόλει καὶ τὰ αὑτῆς ἔχειν ἀσφαλῶς καὶ Φίλιππον
  τιμωρήσασθαι, καὶ μάλ᾿ ἀκριβῶς οἶδα· ἐπ᾿ ἐμοῦ γάρ, οὐ πάλαι
  γέγονεν ταῦτ᾿ ἀμφότερα· νῦν μέντοι πέπεισμαι τοῦθ᾿ ἱκανὸν
  προλαβεῖν ἡμῖν εἶναι τὴν πρώτην, ὅπως τοὺς συμμάχους
  σώσομεν. ἐὰν γὰρ τοῦτο βεβαίως ὑπάρξῃ, τότε καὶ περὶ τοῦ
  τίνα τιμωρήσεταί τις καὶ ὃν τρόπον ἐξέσται σκοπεῖν· πρὶν δὲ
  τὴν ἀρχὴν ὀρθῶς ὑποθέσθαι, μάταιον ἡγοῦμαι περὶ τῆς
  τελευτῆς ὁντινοῦν ποιεῖσθαι λόγον.

  Δημοσθένους, Γ´ ᾿Ολυνθιακὸς

Georgian:

  From a Unicode conference invitation:

  გთხოვთ ახლავე გაიაროთ რეგისტრაცია Unicode-ის მეათე საერთაშორისო
  კონფერენციაზე დასასწრებად, რომელიც გაიმართება 10-12 მარტს,
  ქ. მაინცში, გერმანიაში. კონფერენცია შეჰკრებს ერთად მსოფლიოს
  ექსპერტებს ისეთ დარგებში როგორიცაა ინტერნეტი და Unicode-ი,
  ინტერნაციონალიზაცია და ლოკალიზაცია, Unicode-ის გამოყენება
  ოპერაციულ სისტემებსა, და გამოყენებით პროგრამებში, შრიფტებში,
  ტექსტების დამუშავებასა და მრავალენოვან კომპიუტერულ სისტემებში.

Russian:

  From a Unicode conference invitation:

  Зарегистрируйтесь сейчас на Десятую Международную Конференцию по
  Unicode, которая состоится 10-12 марта 1997 года в Майнце в Германии.
  Конференция соберет широкий круг экспертов по  вопросам глобального
  Интернета и Unicode, локализации и интернационализации, воплощению и
  применению Unicode в различных операционных системах и программных
  приложениях, шрифтах, верстке и многоязычных компьютерных системах.

Thai (UCS Level 2):

  Excerpt from a poetry on The Romance of The Three Kingdoms (a Chinese
  classic 'San Gua'):

  [----------------------------|------------------------]
    ๏ แผ่นดินฮั่นเสื่อมโทรมแสนสังเวช  พระปกเกศกองบู๊กู้ขึ้นใหม่
  สิบสองกษัตริย์ก่อนหน้าแลถัดไป       สององค์ไซร้โง่เขลาเบาปัญญา
    ทรงนับถือขันทีเป็นที่พึ่ง           บ้านเมืองจึงวิปริตเป็นนักหนา
  โฮจิ๋นเรียกทัพทั่วหัวเมืองมา         หมายจะฆ่ามดชั่วตัวสำคัญ
    เหมือนขับไสไล่เสือจากเคหา      รับหมาป่าเข้ามาเลยอาสัญ
  ฝ่ายอ้องอุ้นยุแยกให้แตกกัน          ใช้สาวนั้นเป็นชนวนชื่นชวนใจ
    พลันลิฉุยกุยกีกลับก่อเหตุ          ช่างอาเพศจริงหนาฟ้าร้องไห้
  ต้องรบราฆ่าฟันจนบรรลัย           ฤๅหาใครค้ำชูกู้บรรลังก์ ฯ

  (The above is a two-column text. If combining characters are handled
  correctly, the lines of the second column should be aligned with the
  | character above.)

Ethiopian:

  Proverbs in the Amharic language:

  ሰማይ አይታረስ ንጉሥ አይከሰስ።
  ብላ ካለኝ እንደአባቴ በቆመጠኝ።
  ጌጥ ያለቤቱ ቁምጥና ነው።
  ደሀ በሕልሙ ቅቤ ባይጠጣ ንጣት በገደለው።
  የአፍ ወለምታ በቅቤ አይታሽም።
  አይጥ በበላ ዳዋ ተመታ።
  ሲተረጉሙ ይደረግሙ።
  ቀስ በቀስ፥ ዕንቁላል በእግሩ ይሄዳል።
  ድር ቢያብር አንበሳ ያስር።
  ሰው እንደቤቱ እንጅ እንደ ጉረቤቱ አይተዳደርም።
  እግዜር የከፈተውን ጉሮሮ ሳይዘጋው አይድርም።
  የጎረቤት ሌባ፥ ቢያዩት ይስቅ ባያዩት ያጠልቅ።
  ሥራ ከመፍታት ልጄን ላፋታት።
  ዓባይ ማደሪያ የለው፥ ግንድ ይዞ ይዞራል።
  የእስላም አገሩ መካ የአሞራ አገሩ ዋርካ።
  ተንጋሎ ቢተፉ ተመልሶ ባፉ።
  ወዳጅህ ማር ቢሆን ጨርስህ አትላሰው።
  እግርህን በፍራሽህ ልክ ዘርጋ።

Runes:

  ᚻᛖ ᚳᚹᚫᚦ ᚦᚫᛏ ᚻᛖ ᛒᚢᛞᛖ ᚩᚾ ᚦᚫᛗ ᛚᚪᚾᛞᛖ ᚾᚩᚱᚦᚹᛖᚪᚱᛞᚢᛗ ᚹᛁᚦ ᚦᚪ ᚹᛖᛥᚫ

  (Old English, which transcribed into Latin reads 'He cwaeth that he
  bude thaem lande northweardum with tha Westsae.' and means 'He said
  that he lived in the northern land near the Western Sea.')

Braille:

  ⡌⠁⠧⠑ ⠼⠁⠒  ⡍⠜⠇⠑⠹⠰⠎ ⡣⠕⠌

  ⡍⠜⠇⠑⠹ ⠺⠁⠎ ⠙⠑⠁⠙⠒ ⠞⠕ ⠃⠑⠛⠔ ⠺⠊⠹⠲ ⡹⠻⠑ ⠊⠎ ⠝⠕ ⠙⠳⠃⠞
  ⠱⠁⠞⠑⠧⠻ ⠁⠃⠳⠞ ⠹⠁⠞⠲ ⡹⠑ ⠗⠑⠛⠊⠌⠻ ⠕⠋ ⠙⠊⠎ ⠃⠥⠗⠊⠁⠇ ⠺⠁⠎
  ⠎⠊⠛⠝⠫ ⠃⠹ ⠹⠑ ⠊⠇⠻⠛⠹⠍⠁⠝⠂ ⠹⠑ ⠊⠇⠻⠅⠂ ⠹⠑ ⠥⠝⠙⠻⠞⠁⠅⠻⠂
  ⠁⠝⠙ ⠹⠑ ⠡⠊⠑⠋ ⠍⠳⠗⠝⠻⠲ ⡎⠊⠗⠕⠕⠛⠑ ⠎⠊⠛⠝⠫ ⠊⠞⠲ ⡁⠝⠙
  ⡎⠊⠗⠕⠕⠛⠑⠰⠎ ⠝⠁⠍⠑ ⠺⠁⠎ ⠛⠕⠕⠙ ⠥⠏⠕⠝ ⠰⡡⠁⠝⠛⠑⠂ ⠋⠕⠗ ⠁⠝⠹⠹⠔⠛ ⠙⠑ 
  ⠡⠕⠎⠑ ⠞⠕ ⠏⠥⠞ ⠙⠊⠎ ⠙⠁⠝⠙ ⠞⠕⠲

  ⡕⠇⠙ ⡍⠜⠇⠑⠹ ⠺⠁⠎ ⠁⠎ ⠙⠑⠁⠙ ⠁⠎ ⠁ ⠙⠕⠕⠗⠤⠝⠁⠊⠇⠲

  ⡍⠔⠙⠖ ⡊ ⠙⠕⠝⠰⠞ ⠍⠑⠁⠝ ⠞⠕ ⠎⠁⠹ ⠹⠁⠞ ⡊ ⠅⠝⠪⠂ ⠕⠋ ⠍⠹
  ⠪⠝ ⠅⠝⠪⠇⠫⠛⠑⠂ ⠱⠁⠞ ⠹⠻⠑ ⠊⠎ ⠏⠜⠞⠊⠊⠥⠇⠜⠇⠹ ⠙⠑⠁⠙ ⠁⠃⠳⠞
  ⠁ ⠙⠕⠕⠗⠤⠝⠁⠊⠇⠲ ⡊ ⠍⠊⠣⠞ ⠙⠁⠧⠑ ⠃⠑⠲ ⠔⠊⠇⠔⠫⠂ ⠍⠹⠎⠑⠇⠋⠂ ⠞⠕
  ⠗⠑⠛⠜⠙ ⠁ ⠊⠕⠋⠋⠔⠤⠝⠁⠊⠇ ⠁⠎ ⠹⠑ ⠙⠑⠁⠙⠑⠌ ⠏⠊⠑⠊⠑ ⠕⠋ ⠊⠗⠕⠝⠍⠕⠝⠛⠻⠹ 
  ⠔ ⠹⠑ ⠞⠗⠁⠙⠑⠲ ⡃⠥⠞ ⠹⠑ ⠺⠊⠎⠙⠕⠍ ⠕⠋ ⠳⠗ ⠁⠝⠊⠑⠌⠕⠗⠎ 
  ⠊⠎ ⠔ ⠹⠑ ⠎⠊⠍⠊⠇⠑⠆ ⠁⠝⠙ ⠍⠹ ⠥⠝⠙⠁⠇⠇⠪⠫ ⠙⠁⠝⠙⠎
  ⠩⠁⠇⠇ ⠝⠕⠞ ⠙⠊⠌⠥⠗⠃ ⠊⠞⠂ ⠕⠗ ⠹⠑ ⡊⠳⠝⠞⠗⠹⠰⠎ ⠙⠕⠝⠑ ⠋⠕⠗⠲ ⡹⠳
  ⠺⠊⠇⠇ ⠹⠻⠑⠋⠕⠗⠑ ⠏⠻⠍⠊⠞ ⠍⠑ ⠞⠕ ⠗⠑⠏⠑⠁⠞⠂ ⠑⠍⠏⠙⠁⠞⠊⠊⠁⠇⠇⠹⠂ ⠹⠁⠞
  ⡍⠜⠇⠑⠹ ⠺⠁⠎ ⠁⠎ ⠙⠑⠁⠙ ⠁⠎ ⠁ ⠙⠕⠕⠗⠤⠝⠁⠊⠇⠲

  (The first couple of paragraphs of "A Christmas Carol" by Dickens)

Compact font selection example text:

  ABCDEFGHIJKLMNOPQRSTUVWXYZ /0123456789
  abcdefghijklmnopqrstuvwxyz £©µÀÆÖÞßéöÿ
  –—‘“”„†•…‰™œŠŸž€ ΑΒΓΔΩαβγδω АБВГДабвгд
  ∀∂∈ℝ∧∪≡∞ ↑↗↨↻⇣ ┐┼╔╘░►☺♀ ﬁ�⑀₂ἠḂӥẄɐː⍎אԱა

Greetings in various languages:

  Hello world, Καλημέρα κόσμε, コンニチハ

Box drawing alignment tests:                                          █
                                                                      ▉
  ╔══╦══╗  ┌──┬──┐  ╭──┬──╮  ╭──┬──╮  ┏━━┳━━┓  ┎┒┏┑   ╷  ╻ ┏┯┓ ┌┰┐    ▊ ╱╲╱╲╳╳╳
  ║┌─╨─┐║  │╔═╧═╗│  │╒═╪═╕│  │╓─╁─╖│  ┃┌─╂─┐┃  ┗╃╄┙  ╶┼╴╺╋╸┠┼┨ ┝╋┥    ▋ ╲╱╲╱╳╳╳
  ║│╲ ╱│║  │║   ║│  ││ │ ││  │║ ┃ ║│  ┃│ ╿ │┃  ┍╅╆┓   ╵  ╹ ┗┷┛ └┸┘    ▌ ╱╲╱╲╳╳╳
  ╠╡ ╳ ╞╣  ├╢   ╟┤  ├┼─┼─┼┤  ├╫─╂─╫┤  ┣┿╾┼╼┿┫  ┕┛┖┚     ┌┄┄┐ ╎ ┏┅┅┓ ┋ ▍ ╲╱╲╱╳╳╳
  ║│╱ ╲│║  │║   ║│  ││ │ ││  │║ ┃ ║│  ┃│ ╽ │┃  ░░▒▒▓▓██ ┊  ┆ ╎ ╏  ┇ ┋ ▎
  ║└─╥─┘║  │╚═╤═╝│  │╘═╪═╛│  │╙─╀─╜│  ┃└─╂─┘┃  ░░▒▒▓▓██ ┊  ┆ ╎ ╏  ┇ ┋ ▏
  ╚══╩══╝  └──┴──┘  ╰──┴──╯  ╰──┴──╯  ┗━━┻━━┛           └╌╌┘ ╎ ┗╍╍┛ ┋  ▁▂▃▄▅▆▇█
"#;

    #[test]
    fn utf8_sample_ok() {
        assert!(is_utf8(UTF8_SAMPLE_OK.as_bytes()).is_ok());
    }

    const ASCII_SAMPLE_OK: &str = r#"
Lorem ipsum dolor sit amet, alii meliore his te, eos nemore voluptatum temporibus ex. Saepe dicant ponderum an pro. Pro an nemore apeirian volutpat, mei cu erat partem sadipscing. Integre mentitum an mel, te has sale simul percipit, ludus legere conceptam mel cu. Eu mel enim errem, at vim dicit dolore, vim cu everti utroque praesent.

Sea in debitis delectus invidunt, vero dolorum consequat ne duo. Mea id omittam nominavi consequat, agam commune molestie ut vel. Sit simul utamur democritum ex, ne has odio stet, te sed illum dolorem petentium. Eirmod omnesque qui eu. Dolorum detraxit assueverit duo no, eos ex agam illud deseruisse, te mea tibique percipit delicata.

Id eum adhuc errem ridens. Tamquam vulputate intellegat pro ex, id aliquam facilisis cum. Ei vix phaedrum mediocrem honestatis. Sit at error nostrud propriae, brute assentior eam ei. Vis in vero eripuit, pro stet meis civibus ei. Cu duo detracto recusabo salutandi, oblique appetere id eos. Eum id primis detracto, ius no dolor cetero incorrupte.

Eos dicit utroque id, in qui voluptatum scripserit dissentiet. Virtute facilisi nec ei. Natum persecuti posidonium at pri, tale abhorreant eu sit. Autem sadipscing per et, salutatus intellegat per in, erant instructior qui ut. Officiis expetenda usu no, quo legendos conceptam ei. Euismod urbanitas ut his, vide quando audire eam no, vel ut eros mollis maiestatis.

Natum eripuit legendos et sit. Persecuti interpretaris in ius. Consul probatus prodesset ut est. Harum solet pro te, probo intellegat ea nec. Te dolor bonorum mei. Cum id nostrud molestie omittantur.

Vocent sadipscing comprehensam has te. Mei nibh vivendo ne, sonet labores ut sea. Debitis scriptorem per ex. Mea eu velit efficiendi, bonorum delicatissimi nec at.

Ex has hinc mediocrem. Sea eu sumo conclusionemque, ut sumo choro has. Vim eu postea mnesarchum. In has fugit deserunt, in euripidis voluptaria nam. Pri no illum sadipscing, oratio luptatum instructior et his. Odio modus erroribus his te, maluisset conceptam ullamcorper vis eu, an quem scribentur per.

Eos facilisis democritum te, ex usu eripuit fuisset imperdiet. Vim nonumes philosophia et, esse iudico sea te. Ei usu tollit deleniti. Eam malis nemore no, ut rebum legendos assueverit vix. At qui mollis definiebas, agam delicata scripserit id eam. Essent suscipit accusamus usu no, sed ut partem democritum.

Te mei vide labitur, ubique omnesque philosophia has te. An mei pertinax abhorreant signiferumque, pro in aeque propriae voluptua, at eos mutat splendide eloquentiam. Cu purto habeo eos. Pri nullam postea te, sint vitae tempor id vim, ut offendit moderatius has. Causae imperdiet concludaturque usu ad, mei facer dicam id. Ne sea quidam omittantur. Dicat tation lucilius ad mel, an usu utinam doctus.

Nam cu case dictas euismod. Vel ex suavitate percipitur. His discere labores ut, quo nibh dissentiet ne. Assum augue accusamus ea eam, eum debet scripserit te, vix aeque persequeris ea. Assum gubergren nec eu, in sed admodum feugait recusabo. Nam cibo perfecto ex, cu mentitum gloriatur cum, etiam antiopam intellegebat et sea.
"#;

    #[test]
    fn ascii_sample_ok() {
        assert!(is_utf8(ASCII_SAMPLE_OK.as_bytes()).is_ok());
    }

    const MOSTLY_ASCII_SAMPLE_OK: &str = r#"
Lorem ipsum dolor sit amet, alii meliore his te, eos nemore voluptatum temporibus ex. Saepe dicant ponderum an pro. Pro an nemore apeirian volutpat, mei cu erat partem sadipscing. Integre mentitum an mel, te has sale simul percipit, ludus legere conceptam mel cu. Eu mel enim errem, at vim dicit dolore, vim cu everti utroque praesent.

Sea in debitis delectus invidunt, vero dolorum consequat ne duo. Mea id omittam nominavi consequat, agam commune molestie ut vel. Sit simul utamur democritum ex, ne has odio stet, te sed illum dolorem petentium. Eirmod omnesque qui eu. Dolorum detraxit assueverit duo no, eos ex agam illud deseruisse, te mea tibique percipit delicata.

Id eum adhuc errem ridens. Tamquam vulputate intellegat pro ex, id aliquam facilisis cum. Ei vix phaedrum mediocrem honestatis. Sit at error nostrud propriae, brute assentior eam ei. Vis in vero eripuit, pro stet meis civibus ei. Cu duo detracto recusabo salutandi, oblique appetere id eos. Eum id primis detracto, ius no dolor cetero incorrupte.

Eos dicit utroque id, in qui voluptatum scripserit dissentiet. Virtute facilisi nec ei. Natum persecuti posidonium at pri, tale abhorreant eu sit. Autem sadipscing per et, salutatus intellegat per in, erant instructior qui ut. Officiis expetenda usu no, quo legendos conceptam ei. Euismod urbanitas ut his, vide quando audire eam no, vel ut eros mollis maiestatis.

Natum eripuit legendos et sit. Persecuti interpretaris in ius. Consul probatus prodesset ut est. Harum solet pro te, probo intellegat ea nec. Te dolor bonorum mei. Cum id nostrud molestie omittantur.

Vocent sadipscing comprehensam has te. Mei nibh vivendo ne, sonet labores ut sea. Debitis scriptorem per ex. Mea eu velit efficiendi, bonorum delicatissimi nec at.

Ex has hinc mediocrem. Sea eu sumo conclusionemque, ut sumo choro has. Vim eu postea mnesarchum. In has fugit deserunt, in euripidis voluptaria nam. Pri no illum sadipscing, oratio luptatum instructior et his. Odio modus erroribus his te, maluisset conceptam ullamcorper vis eu, an quem scribentur per.

Eos facilisis democritum te, ex usu eripuit fuisset imperdiet. Vim nonumes philosophia et, esse iudico sea te. Ei usu tollit deleniti. Eam malis nemore no, ut rebum legendos assueverit vix. At qui mollis definiebas, agam delicata scripserit id eam. Essent suscipit accusamus usu no, ᚻᛖ ᚳᚹᚫᚦ ᚦᚫᛏ ᚻᛖ ᛒᚢᛞᛖ ᚩᚾ ᚦᚫᛗ ᛚᚪᚾᛞᛖ ᚾᚩᚱᚦᚹᛖᚪᚱᛞᚢᛗ ᚹᛁᚦ ᚦᚪ ᚹᛖᛥᚫ sed ut partem democritum.

Te mei vide labitur, ubique omnesque philosophia has te. An mei pertinax abhorreant signiferumque, pro in aeque propriae voluptua, at eos mutat splendide eloquentiam. Cu purto habeo eos. Pri nullam postea te, sint vitae tempor id vim, ut offendit moderatius has. Causae imperdiet concludaturque usu ad, mei facer dicam id. Ne sea quidam omittantur. Dicat tation lucilius ad mel, an usu utinam doctus.

Nam cu case dictas euismod. Vel ex suavitate percipitur. His discere labores ut, quo nibh dissentiet ne. Assum augue accusamus ea eam, eum debet scripserit te, vix aeque persequeris ea. Assum gubergren nec eu, in sed admodum feugait recusabo. Nam cibo perfecto ex, cu mentitum gloriatur cum, etiam antiopam intellegebat et sea.
"#;

    #[test]
    fn mostly_ascii_sample_ok() {
        assert!(is_utf8(MOSTLY_ASCII_SAMPLE_OK.as_bytes()).is_ok());
    }
}
