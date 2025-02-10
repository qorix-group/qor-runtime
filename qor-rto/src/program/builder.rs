// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
#![allow(unused)]

use super::*;

/// Build chain builder
type BuildChainBuilder = Box<dyn Fn(&Vec<Box<dyn Action>>) -> Box<dyn Action>>;

/// A single step in the building process
trait BuildStep {
    fn add(&mut self, tag: Tag, step: Box<dyn BuildStep>) -> RtoResult<()>;
    fn build(&self) -> RtoResult<Box<dyn Action>>;
}

/// A chain of actions to be built into one larger action
struct BuildChain {
    builder: BuildChainBuilder,
    chain: Vec<Box<dyn BuildStep>>,
}

impl BuildChain {
    fn new(builder: BuildChainBuilder) -> Self {
        Self {
            builder,
            chain: Vec::new(),
        }
    }
}

impl BuildStep for BuildChain {
    fn add(&mut self, _tag: Tag, step: Box<dyn BuildStep>) -> RtoResult<()> {
        self.chain.push(step);
        Ok(())
    }

    fn build(&self) -> RtoResult<Box<dyn Action>> {
        let actions = self
            .chain
            .iter()
            .map(|step| step.build())
            .collect::<RtoResult<Vec<Box<dyn Action>>>>()?;
        Ok((self.builder)(&actions))
    }
}

struct BuildSingle {
    builder: Box<dyn Fn() -> Box<dyn Action>>,
}

impl BuildSingle {
    fn new(builder: Box<dyn Fn() -> Box<dyn Action>>) -> Self {
        Self { builder }
    }
}

impl BuildStep for BuildSingle {
    fn add(&mut self, _tag: Tag, _step: Box<dyn BuildStep>) -> RtoResult<()> {
        Err(Error::from_code(crate::rto_errors::PROGRAM_BUILD_FAILED))
    }

    fn build(&self) -> RtoResult<Box<dyn Action>> {
        Ok((self.builder)())
    }
}

struct BuildStart {}

impl BuildStep for BuildStart {
    fn add(&mut self, _tag: Tag, _step: Box<dyn BuildStep>) -> RtoResult<()> {
        Err(Error::from_code(crate::rto_errors::PROGRAM_BUILD_FAILED))
    }

    fn build(&self) -> RtoResult<Box<dyn Action>> {
        Err(Error::FUNCTION_NOT_IMPLEMENTED)
    }
}

/// A program builder is a convenient way to build a program
/// in a script-like manner.
pub struct ProgramBuilder {
    step: Box<dyn BuildStep>,
}

impl ProgramBuilder {
    pub fn new() -> Self {
        Self {
            step: Box::new(BuildStart {}),
        }
    }

    pub fn begin() -> Self {
        Self::new()
    }
}

impl Default for ProgramBuilder {
    fn default() -> Self {
        Self::new()
    }
}
