//! Two-character quantum-number fields used in `.cat` files, ported from
//! `readqn()` in `catutil.c`.
//!
//! | Text | Value |
//! |---|---|
//! | ` 5`, `12`, `-3` | 5, 12, −3 |
//! | `A0`…`Z9` | 100…359 (`A` = 10 tens) |
//! | `a0`…`z9` | −10…−269 (`a` = −1 ten, the digit is also negative) |
//! | blank | 0 |

/// Decodes `n` two-character fields from `s`. Missing or blank fields are 0.
/// As in `readqn`, decoding stops (the rest are 0) at a field whose second
/// character is not a digit or blank.
pub fn decode(s: &str, n: usize) -> Vec<i32> {
    let b = s.as_bytes();
    let mut out = vec![0; n];
    for (i, slot) in out.iter_mut().enumerate() {
        let (Some(&first), Some(&second)) = (b.get(2 * i), b.get(2 * i + 1)) else { break };
        if second == b' ' {
            continue;
        }
        if !second.is_ascii_digit() {
            break;
        }
        *slot = decode_pair(first, second);
    }
    out
}

fn decode_pair(first: u8, second: u8) -> i32 {
    let units = (second - b'0') as i32;
    match first {
        b' ' => units,
        b'-' => -units,
        b'0'..=b'9' => units + 10 * (first - b'0') as i32,
        b'a'..=b'z' => -units - 10 * ((first - b'a') as i32 + 1),
        b'A'..=b'Z' => units + 10 * ((first - b'A') as i32 + 10),
        _ => 36, // the C code's value for an unknown character
    }
}

/// Encodes one value as a two-character field. Values outside −269…359
/// cannot be written and become `**`.
pub fn encode(v: i32) -> String {
    match v {
        -9..=99 => format!("{:>2}", v),
        100..=359 => {
            let tens = (v / 10 - 10) as u8;
            format!("{}{}", (b'A' + tens) as char, v % 10)
        }
        -269..=-10 => {
            let a = -v;
            let tens = (a / 10 - 1) as u8;
            format!("{}{}", (b'a' + tens) as char, a % 10)
        }
        _ => "**".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_all_encodable_values() {
        for v in -269..=359 {
            let s = encode(v);
            assert_eq!(s.len(), 2, "{v}");
            assert_eq!(decode(&s, 1), vec![v], "{v} -> {s:?}");
        }
    }

    #[test]
    fn decodes_cat_fields() {
        assert_eq!(decode(" 2 1-1 3    ", 6), vec![2, 1, -1, 3, 0, 0]);
        assert_eq!(decode("A3b5", 2), vec![103, -25]);
        assert_eq!(decode(" 1", 3), vec![1, 0, 0]);
    }
}
