// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use crate::base::*;
use qor_core::prelude::*;

use std::{
    fmt::{Debug, Display},
    hash::{Hash, Hasher},
    sync::RwLock,
    time::Duration,
};

/// A VariableStorage contains a single value storage fulfilling the Variable trait.
#[derive(Debug)]
pub struct VariableStorage<T: TypeTag + Scalar + Debug + Copy + Send + Sync> {
    value: RwLock<T>,
}

impl<T: TypeTag + Scalar + Debug + Copy + Send + Sync> Display for VariableStorage<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self.value.read().unwrap())
    }
}

impl<T: TypeTag + Scalar + Debug + PartialEq + Send + Sync> PartialEq for VariableStorage<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value.read().unwrap().eq(&other.value.read().unwrap())
    }
}

impl<T: TypeTag + Scalar + Debug + Copy + Send + Sync> Clone for VariableStorage<T> {
    fn clone(&self) -> Self {
        VariableStorage {
            value: RwLock::new(*self.value.read().unwrap()),
        }
    }
}

impl<T: TypeTag + Scalar + Debug + Hash + Send + Sync> Hash for VariableStorage<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.read().unwrap().hash(state);
    }
}

impl<T: TypeTag + Scalar + Debug + Copy + PartialEq + Send + Sync> VariableStorage<T> {
    pub fn new(value: T) -> Self {
        VariableStorage {
            value: RwLock::new(value),
        }
    }
}

impl<T: TypeTag + Scalar + Debug + Send + Sync> LValue<T> for VariableStorage<T> {
    fn assign(&self, value: T) -> Result<T, EvalError> {
        *self.value.write().unwrap() = value;
        Ok(value)
    }
}

impl<T: TypeTag + Scalar + Debug + Send + Sync> RValue<T> for VariableStorage<T> {
    fn eval(&self) -> Result<T, EvalError> {
        Ok(*self.value.read().unwrap())
    }
}

impl<T: TypeTag + Scalar + Debug + Send + Sync> Variable<T> for VariableStorage<T> {}

/// A StateStorage is a state with a variable value that implements the State trait
/// and updates new values upon update only.
#[derive(Debug)]
pub struct StateStorage<T: TypeTag + Scalar + Debug + PartialEq + Copy + Send + Sync> {
    value: VariableStorage<T>,
    new_value: VariableStorage<T>,
}

impl<T: TypeTag + Scalar + Debug + PartialEq + Copy + Send + Sync> Display for StateStorage<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{} ({})", self.value, self.new_value)
    }
}

impl<T: TypeTag + Scalar + Debug + PartialEq + Copy + Send + Sync> StateStorage<T> {
    pub fn new(value: T) -> Self {
        StateStorage {
            value: VariableStorage::new(value),
            new_value: VariableStorage::new(value),
        }
    }

    /// Reset the state to the value given
    pub fn reset(&mut self, value: T) {
        let _ = self.new_value.assign(value);
        let _ = self.value.assign(value);
    }
}

impl<T: TypeTag + Scalar + Debug + PartialEq + Copy + Send + Sync> LValue<T> for StateStorage<T> {
    fn assign(&self, value: T) -> Result<T, EvalError> {
        self.new_value.assign(value)
    }
}

impl<T: TypeTag + Scalar + Debug + PartialEq + Copy + Send + Sync> RValue<T> for StateStorage<T> {
    fn eval(&self) -> Result<T, EvalError> {
        self.value.eval()
    }
}

impl<T: TypeTag + Scalar + Debug + PartialEq + Copy + Send + Sync> Variable<T> for StateStorage<T> {}

impl<T: TypeTag + Scalar + Debug + PartialEq + Copy + Send + Sync> State<T> for StateStorage<T> {
    fn update(&mut self, _delta: &Duration) -> Result<T, EvalError> {
        self.value.assign(self.new_value.eval()?)
    }
}
