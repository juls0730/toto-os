use core::fmt::Display;

use alloc::{string::String, vec::Vec};

#[derive(Clone, Copy, Debug)]
pub struct Uuid {
    pub a: u32,
    pub b: u16,
    pub c: u16,
    pub d: [u8; 8],
}

impl From<[u8; 16]> for Uuid {
    fn from(value: [u8; 16]) -> Self {
        let a = u32::from_le_bytes(value[0..4].try_into().unwrap());
        let b = u16::from_le_bytes(value[4..6].try_into().unwrap());
        let c = u16::from_le_bytes(value[6..8].try_into().unwrap());
        let d = value[8..16].try_into().unwrap();

        return Self { a, b, c, d };
    }
}

impl PartialEq for Uuid {
    fn eq(&self, other: &Self) -> bool {
        return self.a == other.a && self.b == other.b && self.c == other.c && self.d == other.d;
    }
}

impl PartialEq<&str> for Uuid {
    fn eq(&self, other: &&str) -> bool {
        let parts = other.split('-').collect::<Vec<&str>>();

        if parts.len() != 5 {
            return false;
        }

        let a = u32::from_str_radix(parts[0], 16);
        let b = u16::from_str_radix(parts[1], 16);
        let c = u16::from_str_radix(parts[2], 16);

        if a.is_err() || b.is_err() || c.is_err() {
            return false;
        }

        let d = {
            let part = [parts[3], parts[4]].concat();

            let mut d_vec = Vec::new();
            let d_parts = part
                .trim()
                .chars()
                .collect::<Vec<_>>()
                .chunks(2)
                .map(|chunk| chunk.iter().collect())
                .collect::<Vec<String>>();

            for d_part in d_parts {
                let d_part = u8::from_str_radix(&d_part, 16);

                if d_part.is_err() {
                    return false;
                }

                d_vec.push(d_part.unwrap());
            }

            d_vec
        };

        let d: &[u8] = &d;

        let d: Result<[u8; 8], _> = d.try_into();

        if d.is_err() {
            return false;
        }

        let uuid = Uuid {
            a: a.unwrap(),
            b: b.unwrap(),
            c: c.unwrap(),
            d: d.unwrap(),
        };

        return self == &uuid;
    }
}

impl Display for Uuid {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:X}-{:X}-{:X}-", self.a, self.b, self.c)?;

        for &byte in &self.d {
            write!(f, "{:02X}", byte)?;
        }

        Ok(())
    }
}
