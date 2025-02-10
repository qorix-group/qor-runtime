// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use super::*;

// TODO: Add std::atomic::* to marker traits.

//
// TypeTags for internal types
//

impl TypeTag for bool {
    const TYPE_TAG: Tag = Tag::new(*b"@bool___");
}

impl TypeTag for u8 {
    const TYPE_TAG: Tag = Tag::new(*b"@u8_____");
}
impl TypeTag for i8 {
    const TYPE_TAG: Tag = Tag::new(*b"@i8_____");
}

impl TypeTag for u16 {
    const TYPE_TAG: Tag = Tag::new(*b"@u16____");
}

impl TypeTag for i16 {
    const TYPE_TAG: Tag = Tag::new(*b"@i16____");
}

impl TypeTag for u32 {
    const TYPE_TAG: Tag = Tag::new(*b"@u32____");
}

impl TypeTag for i32 {
    const TYPE_TAG: Tag = Tag::new(*b"@i32____");
}

impl TypeTag for u64 {
    const TYPE_TAG: Tag = Tag::new(*b"@u64____");
}
impl TypeTag for i64 {
    const TYPE_TAG: Tag = Tag::new(*b"@i64____");
}

impl TypeTag for u128 {
    const TYPE_TAG: Tag = Tag::new(*b"@u128___");
}
impl TypeTag for i128 {
    const TYPE_TAG: Tag = Tag::new(*b"@i128___");
}

impl TypeTag for usize {
    const TYPE_TAG: Tag = Tag::new(*b"@usize__");
}
impl TypeTag for isize {
    const TYPE_TAG: Tag = Tag::new(*b"@isize__");
}

impl TypeTag for crate::types::f16 {
    const TYPE_TAG: Tag = Tag::new(*b"@f16____");
}

impl TypeTag for crate::types::bf16 {
    const TYPE_TAG: Tag = Tag::new(*b"@bf16___");
}

impl TypeTag for f32 {
    const TYPE_TAG: Tag = Tag::new(*b"@f32____");
}

impl TypeTag for f64 {
    const TYPE_TAG: Tag = Tag::new(*b"@f64____");
}

impl TypeTag for char {
    const TYPE_TAG: Tag = Tag::new(*b"@char___");
}

impl TypeTag for str {
    const TYPE_TAG: Tag = Tag::new(*b"@str____");
}

impl TypeTag for String {
    const TYPE_TAG: Tag = Tag::from_str("std::string::String");
}

impl<const N: usize> TypeTag for StrConst<N> {
    const TYPE_TAG: Tag = Tag::new(*b"@Str____");
}

// Arrays
impl<T, const N: usize> TypeTag for [T; N]
where
    T: TypeTag,
{
    const TYPE_TAG: Tag = Tag::new(*b"@[]_____")
        .with_added_tag(T::TYPE_TAG)
        .with_added_tag(Tag::from_raw(N as u64));
}
