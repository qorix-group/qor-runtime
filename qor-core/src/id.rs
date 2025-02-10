// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use std::hash::{Hash, Hasher};

use crate::hash::Fnv1a64ConstHasher;
use crate::types::{Coherent, Lockfree, Reloc, TypeTag};

/// Id type
///
/// An Id is a number to identify an indexing type of identification.
/// In differentiation to simple usize indexes Id also knows an invalid value.
/// Throughout the Qor frameworks Id is preferred over flat index types.
///
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(transparent)]
pub struct Id {
    index: usize,
}

impl Default for Id {
    fn default() -> Self {
        Id::invalid()
    }
}

impl TypeTag for Id {
    const TYPE_TAG: Tag = Tag::new(*b"@Id_____");
}

impl Coherent for Id {}
unsafe impl Reloc for Id {}
unsafe impl Lockfree for Id {}

impl Id {
    const INVALID: usize = usize::MAX;

    /// Create a new id with the given index number
    pub const fn new(index: usize) -> Self {
        Id { index }
    }

    /// Create a new invalid Id
    pub const fn invalid() -> Self {
        Id {
            index: Self::INVALID,
        }
    }

    /// Get the Id as index
    pub const fn index(&self) -> usize {
        if self.is_valid() {
            self.index
        } else {
            panic!("invalid index");
        }
    }

    /// Check validity of Id
    pub const fn is_valid(&self) -> bool {
        self.index != Id::INVALID
    }

    /// Invalidate the Id
    pub fn invalidate(&mut self) {
        self.index = Id::INVALID;
    }
}

impl From<u64> for Id {
    fn from(value: u64) -> Self {
        Id::new(value as usize)
    }
}

impl Hash for Id {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.index as u64);
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_valid() {
            write!(f, "#{:03}", self.index)
        } else {
            write!(f, "#invalid")
        }
    }
}

/// A name tag
///
/// A tag is a 64 bit identifier with a capacity to be human readable. It is either
///
/// - An eight character string or
/// - A hash value calculated from a longer string
///
#[derive(Clone, Copy, PartialEq)]
#[repr(transparent)]
pub struct Tag {
    tag: u64,
}

impl Default for Tag {
    fn default() -> Self {
        Tag::invalid()
    }
}

impl TypeTag for Tag {
    const TYPE_TAG: Tag = Tag::new(*b"@Tag____");
}

impl Coherent for Tag {}
unsafe impl Reloc for Tag {}
unsafe impl Lockfree for Tag {}

impl Tag {
    const INVALID: u64 = u64::MAX;
    const HASH_BIT: u64 = 1 << 63;
    const HASH_MASK: u64 = Self::HASH_BIT - 1;

    /// Create a new tag from a constant.
    ///
    /// The constant is regarded as text and gets the hash-bit reset.
    #[inline]
    pub const fn new(tag: [u8; 8]) -> Self {
        Tag {
            tag: u64::from_le_bytes(tag) & Self::HASH_MASK,
        }
    }

    /// Create a new tag from a raw value
    ///
    /// The value is not altered.
    #[inline]
    pub const fn from_raw(tag: u64) -> Self {
        Tag { tag }
    }

    /// Create a new tag from a constant value
    ///
    /// The value is regarded as "non-text" and gets the hash-bit set.
    #[inline]
    pub const fn from_const(tag: u64) -> Self {
        Tag {
            tag: tag | Self::HASH_BIT,
        }
    }

    /// Create a new tag by hashing the passed string.
    #[inline]
    pub const fn from_str(tag: &str) -> Self {
        Self::from_const(Fnv1a64ConstHasher::from_const(tag.as_bytes()).finish())
    }

    /// Create a new invalid tag
    #[inline]
    pub const fn invalid() -> Self {
        Self::from_raw(Self::INVALID)
    }

    /// Create a new tag from existing by adding another tag
    ///
    /// This always causes the tag to be hashed.
    #[inline]
    pub const fn with_added_tag(&self, tag: Tag) -> Self {
        Self::from_const(
            Fnv1a64ConstHasher::from_seed(self.tag)
                .extended(&tag.value().to_le_bytes())
                .finish(),
        )
    }

    /// Create a new tag from existing by adding a string
    ///
    /// This always causes the tag to be hashed.
    #[inline]
    pub const fn with_added_str(&self, tag: &str) -> Self {
        Self::from_const(
            Fnv1a64ConstHasher::from_seed(self.tag)
                .extended(tag.as_bytes())
                .finish(),
        )
    }

    /// Check if tag is valid
    #[inline]
    pub const fn is_valid(&self) -> bool {
        self.tag != Self::INVALID
    }

    /// Get the tag value
    #[inline]
    pub const fn value(&self) -> u64 {
        self.tag
    }

    /// Invalidate the tag
    #[inline]
    pub fn invalidate(&mut self) {
        self.tag = Self::INVALID;
    }
}

impl From<u64> for Tag {
    fn from(value: u64) -> Self {
        Tag { tag: value }
    }
}

impl From<&str> for Tag {
    /// Create the Tag from a string value.
    ///
    /// This uses a hashing function to  
    fn from(value: &str) -> Self {
        Self::from_str(value)
    }
}

impl From<Tag> for u64 {
    fn from(val: Tag) -> Self {
        val.tag
    }
}

impl Eq for Tag {
    fn assert_receiver_is_total_eq(&self) {}
}

impl Hash for Tag {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.tag);
    }
}

impl std::fmt::Debug for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_valid() {
            let bytes = self.tag.to_le_bytes();

            if bytes.iter().all(|&c| (32..=127).contains(&c)) {
                unsafe {
                    write!(
                        f,
                        "#{} {:016x}",
                        std::str::from_utf8_unchecked(&bytes),
                        self.tag
                    ) // in debug mode text and hash
                }
            } else {
                write!(f, "#{:016x}", self.tag)
            }
        } else {
            write!(f, "#invalid")
        }
    }
}

impl std::fmt::Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_valid() {
            let bytes = self.tag.to_le_bytes();

            if bytes.iter().all(|&c| (32..=127).contains(&c)) {
                unsafe { write!(f, "#{}", std::str::from_utf8_unchecked(&bytes),) }
            } else {
                write!(f, "#{:016x}", self.tag) // only text without hash
            }
        } else {
            write!(f, "#invalid")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;

    #[test]
    fn id_tests() {
        // direct value
        let id = Id::new(42);
        assert_eq!(id.index(), 42);

        // Display
        let mut buff = String::new();
        write!(&mut buff, "{}", id).unwrap();
        assert_eq!(buff, "#042");

        // Display
        buff.clear();
        write!(&mut buff, "{}", Id::invalid()).unwrap();
        assert_eq!(buff, "#invalid");
    }

    #[test]
    fn tag_tests() {
        // direct value
        assert_eq!(Tag::new(*b"01234567").value(), 0x3736353433323130u64);

        // hash value
        let tag = Tag::from("/root/folder/file.txt");
        assert_eq!(tag.value(), 0xcbf7932f2aa742bc);

        // direct value into string using Display
        let mut buff = String::new();
        write!(&mut buff, "{:?}", Tag::new(*b"mem_heap")).unwrap();
        assert_eq!(buff, "#mem_heap 706165685f6d656d");

        // hash value into string using Display
        buff.clear();
        write!(&mut buff, "{:?}", Tag::from(0x71C0)).unwrap();
        assert_eq!(buff, "#00000000000071c0");

        // invalid value into string using Display
        buff.clear();
        write!(&mut buff, "{:?}", Tag::invalid()).unwrap();
        assert_eq!(buff, "#invalid");
    }
}
