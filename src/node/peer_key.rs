use serde::{Serialize, Deserialize};
use crate::utils::utils::Distance;
use crate::utils::utils::ByteRep;
use crate::impl_ByteRep;
use rand;
use std::fmt::Binary;

impl_ByteRep!(for Key);

/// A tuple struct containing a u8 array representation of a 256 bit key
#[derive(Ord, PartialOrd, PartialEq, Eq, Clone, Hash, Serialize, Deserialize, Default, Copy, Debug)]
pub struct Key([u8; 32]);

impl Key {

    /// Generates a new key given the inner array
    pub fn new(v: [u8; 32]) -> Self {
        Key(v)
    }

    /// Generates a key from a slice of u8 bytes
    pub fn from_slice(v: &[u8]) -> Key {
        serde_json::from_slice(v).unwrap()
    }

    /// gets the inner array of u8 bytes
    pub fn get_key(&self) -> [u8; 32] {
        self.0
    }

    /// generates a random key
    pub fn rand() -> Self {
        let mut ret = Key([0; 32]);
        ret.0.iter_mut().for_each(|k| {
            *k = rand::random::<u8>();
        });

        ret
    }

    /// generates a random key within a range
    pub fn rand_in_range(idx: usize) -> Self {
        let mut ret = Key::rand();
        let bytes = idx / 8;
        let bit = idx % 8;
        (0..bytes).into_iter().for_each(|i| {
            ret.0[i] = 0;
        });
        ret.0[bytes] &= 0xFF >>(bit);
        ret.0[bytes] |= 1 << (8 - bit - 1);

        ret
    }

    /// gets the binary string representation of the key's inner byte array
    pub fn get_binary(&self) -> String {
        format!("{:b}", self)
    }

    /// returns a subset of the binary representation of the key
    /// 
    /// # Arguments
    /// 
    /// * size - the length of the prefix to return
    /// 
    pub fn get_prefix(&self, size: usize) -> String {
        let binary = self.get_binary();
        let mut prefix = String::new();
        if size > 0 {
            prefix.push_str(&binary[0..=size]);
            prefix
        } else {
            prefix.push_str(&binary.chars().next().unwrap().to_string());
            prefix
        }
    }
}

impl Distance for Key {
    type Output = Key;

    fn xor(&self, other: Key) -> Key {
        let mut inner = [0; 32];
        inner.iter_mut().enumerate().for_each(|(idx, byte)| {
            *byte = self.0[idx] ^ other.0[idx];
        });

        Key(inner)
    }

    fn leading_zeros(&self) -> usize {
        let mut n = 0;
        for i in 0..32 {
            if self.0[i] == 0 {
                n += 8
            } else {
                return n + self.0[i].leading_zeros() as usize;
            }
        };
        n
    }

    fn leading_ones(&self) -> usize {
        let mut n = 0;
        for i in 0..32 {
            if self.0[i] != 0 {
                if self.0[i].leading_ones() as usize == 8 {
                    n += 8
                } else {
                    return n + self.0[i].leading_ones() as usize
                }
            }
        }

        0
    }
}

impl Binary for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let val = self.get_key();
        val.iter().for_each(|byte| {
            write!(f, "{:b}", byte);
        });

        Ok(())
    }
}