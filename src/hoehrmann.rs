//! Björn Höhrmann’s algorithm:
//!
//! http://bjoern.hoehrmann.de/utf-8/decoder/dfa/

use ::{Utf8Error, Utf8ErrorImpl};

const UTF8_ACCEPT: u8 = 0;
const UTF8_REJECT: u8 = 12;

#[cfg_attr(rustfmt, rustfmt_skip)]
const UTF8D: [u8; 364] = [
    // The first part of the table maps bytes to character classes that
    // to reduce the size of the transition table and create bitmasks.
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,  9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,
    7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,  7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,
    8,8,2,2,2,2,2,2,2,2,2,2,2,2,2,2,  2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
    10,3,3,3,3,3,3,3,3,3,3,3,3,4,3,3, 11,6,6,6,5,8,8,8,8,8,8,8,8,8,8,8,

    // The second part is a transition table that maps a combination
    // of a state of the automaton and a character class to a state.
    0,12,24,36,60,96,84,12,12,12,48,72, 12,12,12,12,12,12,12,12,12,12,12,12,
    12, 0,12,12,12,12,12, 0,12, 0,12,12, 12,24,12,12,12,12,12,24,12,24,12,12,
    12,12,12,12,12,12,12,24,12,12,12,12, 12,24,12,12,12,12,12,12,12,24,12,12,
    12,12,12,12,12,12,12,36,12,36,12,12, 12,36,12,12,12,12,12,36,12,36,12,12,
    12,36,12,12,12,12,12,12,12,12,12,12,
];

#[inline]
unsafe fn decode(state: u8, byte: u8) -> u8 {
    *UTF8D.get_unchecked(256_usize + state as usize + UTF8D[byte as usize] as usize)
}

#[inline]
pub fn is_utf8(x: &[u8]) -> Result<(), Utf8Error> {
    let mut s = UTF8_ACCEPT;
    let mut first_not_ok = 0;
    for i in 0..x.len() {
        s = unsafe { decode(s, *x.get_unchecked(i)) };
        match s {
            UTF8_ACCEPT => { first_not_ok = i + 1; },
            UTF8_REJECT => return Err(Utf8ErrorImpl(first_not_ok, Some(1)).get()),
            _ => {},
        }
    }
    match s {
        UTF8_ACCEPT => Ok(()),
        _ => return Err(Utf8ErrorImpl(first_not_ok, None).get()),
    }
}
