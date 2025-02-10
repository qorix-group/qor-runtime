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
    sync::Arc,
};

/// Constant is an RValue that always evaluates to the same value
#[derive(Debug, Clone)]
pub struct Constant<T>
where
    T: TypeTag + Clone + Debug + Send + Sync,
{
    value: T,
}

impl<T> Display for Constant<T>
where
    T: TypeTag + Clone + Debug + Send + Sync,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "const {:?}", self.value)
    }
}

impl<T> Constant<T>
where
    T: TypeTag + Clone + Debug + Send + Sync,
{
    pub const fn new(value: T) -> Self {
        Self { value }
    }
}

impl<T> RValue<T> for Constant<T>
where
    T: TypeTag + Clone + Debug + Send + Sync,
{
    fn eval(&self) -> Result<T, EvalError> {
        Ok(self.value.clone())
    }
}

/// A Predicate is a user-defined function that evaluates to true or false
pub struct Predicate {
    predicate: Box<dyn Fn() -> Result<bool, EvalError> + Send + Sync + 'static>,
}

impl Predicate {
    #[inline(always)]
    pub fn new<F>(predicate: F) -> Self
    where
        F: Fn() -> Result<bool, EvalError> + Send + Sync + 'static,
    {
        Predicate {
            predicate: Box::new(predicate),
        }
    }
}

impl Display for Predicate {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "predicate")
    }
}

impl Debug for Predicate {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "predicate")
    }
}

impl RValue<bool> for Predicate {
    fn eval(&self) -> Result<bool, EvalError> {
        (self.predicate)()
    }
}

//
// Operators
//

/// Eq is an RValue that compares two RValues and evaluates to true if both are equal
/// `std::cmp::PartialEq` must be defined for `T`.
#[derive(Debug, Clone)]
pub struct Eq<T, U>
where
    T: std::cmp::PartialEq + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    lhs: Arc<dyn RValue<T>>,
    rhs: Arc<dyn RValue<T>>,
    _phantom: std::marker::PhantomData<U>,
}

impl<T, U> Display for Eq<T, U>
where
    T: std::cmp::PartialEq + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({:?}) == ({:?})", self.lhs, self.rhs)
    }
}

impl<T, U> Eq<T, U>
where
    T: std::cmp::PartialEq + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    /// Creates a new Not expression
    pub fn new(lhs: Arc<dyn RValue<T>>, rhs: Arc<dyn RValue<T>>) -> Self {
        Self {
            lhs,
            rhs,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, U> RValue<bool> for Eq<T, U>
where
    T: std::cmp::PartialEq + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn eval(&self) -> Result<bool, EvalError> {
        Ok(self.lhs.eval()? == self.rhs.eval()?)
    }
}

/// Neq is an RValue that compares two RValues and evaluates to false if both are equal
/// `std::cmp::PartialEq` must be defined for `T`.
#[derive(Debug, Clone)]
pub struct Neq<T, U>
where
    T: std::cmp::PartialEq + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    lhs: Arc<dyn RValue<T>>,
    rhs: Arc<dyn RValue<T>>,
    _phantom: std::marker::PhantomData<U>,
}

impl<T, U> Display for Neq<T, U>
where
    T: std::cmp::PartialEq + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({:?}) != ({:?})", self.lhs, self.rhs)
    }
}

impl<T, U> Neq<T, U>
where
    T: std::cmp::PartialEq + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    /// Creates a new Not expression
    pub fn new(lhs: Arc<dyn RValue<T>>, rhs: Arc<dyn RValue<T>>) -> Self {
        Self {
            lhs,
            rhs,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, U> RValue<bool> for Neq<T, U>
where
    T: std::cmp::PartialEq + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn eval(&self) -> Result<bool, EvalError> {
        Ok(self.lhs.eval()? != self.rhs.eval()?)
    }
}

/// Lt is an RValue that compares two RValues and evaluates to true if lhs is less than rhs
/// `std::cmp::PartialOrd` must be defined for `T`.
#[derive(Debug, Clone)]
pub struct Lt<T, U>
where
    T: std::cmp::PartialOrd + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    lhs: Arc<dyn RValue<T>>,
    rhs: Arc<dyn RValue<T>>,
    _phantom: std::marker::PhantomData<U>,
}

impl<T, U> Display for Lt<T, U>
where
    T: std::cmp::PartialOrd + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({:?}) < ({:?})", self.lhs, self.rhs)
    }
}

impl<T, U> Lt<T, U>
where
    T: std::cmp::PartialOrd + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    /// Creates a new Not expression
    pub fn new(lhs: Arc<dyn RValue<T>>, rhs: Arc<dyn RValue<T>>) -> Self {
        Self {
            lhs,
            rhs,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, U> RValue<bool> for Lt<T, U>
where
    T: std::cmp::PartialOrd + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn eval(&self) -> Result<bool, EvalError> {
        Ok(self.lhs.eval()? < self.rhs.eval()?)
    }
}

/// Lte is an RValue that compares two RValues and evaluates to true if lhs is less or equal than rhs
/// `std::cmp::PartialOrd` must be defined for `T`.
#[derive(Debug, Clone)]
pub struct Lte<T, U>
where
    T: std::cmp::PartialOrd + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    lhs: Arc<dyn RValue<T>>,
    rhs: Arc<dyn RValue<T>>,
    _phantom: std::marker::PhantomData<U>,
}

impl<T, U> Display for Lte<T, U>
where
    T: std::cmp::PartialOrd + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({:?}) <= ({:?})", self.lhs, self.rhs)
    }
}

impl<T, U> Lte<T, U>
where
    T: std::cmp::PartialOrd + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    /// Creates a new Not expression
    pub fn new(lhs: Arc<dyn RValue<T>>, rhs: Arc<dyn RValue<T>>) -> Self {
        Self {
            lhs,
            rhs,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, U> RValue<bool> for Lte<T, U>
where
    T: std::cmp::PartialOrd + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn eval(&self) -> Result<bool, EvalError> {
        Ok(self.lhs.eval()? <= self.rhs.eval()?)
    }
}

/// Gt is an RValue that compares two RValues and evaluates to true if lhs is greater than rhs
/// `std::cmp::PartialOrd` must be defined for `T`.
#[derive(Debug, Clone)]
pub struct Gt<T, U>
where
    T: std::cmp::PartialOrd + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    lhs: Arc<dyn RValue<T>>,
    rhs: Arc<dyn RValue<T>>,
    _phantom: std::marker::PhantomData<U>,
}

impl<T, U> Display for Gt<T, U>
where
    T: std::cmp::PartialOrd + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({:?}) > ({:?})", self.lhs, self.rhs)
    }
}

impl<T, U> Gt<T, U>
where
    T: std::cmp::PartialOrd + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    /// Creates a new Not expression
    pub fn new(lhs: Arc<dyn RValue<T>>, rhs: Arc<dyn RValue<T>>) -> Self {
        Self {
            lhs,
            rhs,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, U> RValue<bool> for Gt<T, U>
where
    T: std::cmp::PartialOrd + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn eval(&self) -> Result<bool, EvalError> {
        Ok(self.lhs.eval()? > self.rhs.eval()?)
    }
}

/// Gte is an RValue that compares two RValues and evaluates to true if lhs is greater or equal than rhs
/// `std::cmp::PartialOrd` must be defined for `T`.
#[derive(Debug, Clone)]
pub struct Gte<T, U>
where
    T: std::cmp::PartialOrd + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    lhs: Arc<dyn RValue<T>>,
    rhs: Arc<dyn RValue<T>>,
    _phantom: std::marker::PhantomData<U>,
}

impl<T, U> Display for Gte<T, U>
where
    T: std::cmp::PartialOrd + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({:?}) >= ({:?})", self.lhs, self.rhs)
    }
}

impl<T, U> Gte<T, U>
where
    T: std::cmp::PartialOrd + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    /// Creates a new Not expression
    pub fn new(lhs: Arc<dyn RValue<T>>, rhs: Arc<dyn RValue<T>>) -> Self {
        Self {
            lhs,
            rhs,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, U> RValue<bool> for Gte<T, U>
where
    T: std::cmp::PartialOrd + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn eval(&self) -> Result<bool, EvalError> {
        Ok(self.lhs.eval()? >= self.rhs.eval()?)
    }
}

//
// Operations
//

/// Not is an RValue that inverts another RValue
/// `std::ops::Not` must be defined for `T`.
#[derive(Debug, Clone)]
pub struct Not<T, U>
where
    T: std::ops::Not<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    op: Arc<U>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, U> Display for Not<T, U>
where
    T: std::ops::Not<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "!({:?})", self.op)
    }
}

impl<T, U> Not<T, U>
where
    T: std::ops::Not<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    /// Creates a new Not expression
    pub fn new(op: Arc<U>) -> Self {
        Self {
            op,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, U> RValue<T> for Not<T, U>
where
    T: std::ops::Not<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn eval(&self) -> Result<T, EvalError> {
        match self.op.eval() {
            Ok(value) => Ok(!value),
            Err(err) => Err(err),
        }
    }
}

/// Neg is an RValue that negates another RValue
/// `std::ops::Neg` must be defined for `T`.
#[derive(Debug, Clone)]
pub struct Neg<T, U>
where
    T: std::ops::Neg<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    op: Arc<U>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, U> Display for Neg<T, U>
where
    T: std::ops::Neg<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "-({:?})", self.op)
    }
}

impl<T, U> Neg<T, U>
where
    T: std::ops::Neg<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    /// Creates a new Not expression
    pub fn new(op: Arc<U>) -> Self {
        Self {
            op,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: std::ops::Neg<Output = T> + TypeTag + Debug + Send + Sync, U: RValue<T>> RValue<T>
    for Neg<T, U>
where
    T: std::ops::Neg<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn eval(&self) -> Result<T, EvalError> {
        match self.op.eval() {
            Ok(value) => Ok(-value),
            Err(err) => Err(err),
        }
    }
}

/// Add is an RValue that combines two RValues with an addition.
/// std::ops::Add must be defined for T.
#[derive(Debug, Clone)]
pub struct Add<T, U>
where
    T: std::ops::Add<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    lhs: Arc<U>,
    rhs: Arc<U>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, U> Display for Add<T, U>
where
    T: std::ops::Add<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({:?}) & ({:?})", self.lhs, self.rhs)
    }
}

impl<T, U> Add<T, U>
where
    T: std::ops::Add<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    /// Creates a new Not expression
    pub fn new(lhs: Arc<U>, rhs: Arc<U>) -> Self {
        Self {
            lhs,
            rhs,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, U> RValue<T> for Add<T, U>
where
    T: std::ops::Add<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn eval(&self) -> Result<T, EvalError> {
        Ok(self.lhs.eval()?.add(self.rhs.eval()?))
    }
}

/// Sub is an RValue that combines two RValues with an substraction.
/// std::ops::Sub must be defined for T.
#[derive(Debug, Clone)]
pub struct Sub<T, U>
where
    T: std::ops::Sub<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    lhs: Arc<U>,
    rhs: Arc<U>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, U> Display for Sub<T, U>
where
    T: std::ops::Sub<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({:?}) & ({:?})", self.lhs, self.rhs)
    }
}

impl<T, U> Sub<T, U>
where
    T: std::ops::Sub<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    /// Creates a new Not expression
    pub fn new(lhs: Arc<U>, rhs: Arc<U>) -> Self {
        Self {
            lhs,
            rhs,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, U> RValue<T> for Sub<T, U>
where
    T: std::ops::Sub<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn eval(&self) -> Result<T, EvalError> {
        Ok(self.lhs.eval()?.sub(self.rhs.eval()?))
    }
}

/// Mul is an RValue that combines two RValues with a multiplication.
/// std::ops::Mul must be defined for T.
#[derive(Debug, Clone)]
pub struct Mul<T, U>
where
    T: std::ops::Mul<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    lhs: Arc<U>,
    rhs: Arc<U>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, U> Display for Mul<T, U>
where
    T: std::ops::Mul<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({:?}) & ({:?})", self.lhs, self.rhs)
    }
}

impl<T, U> Mul<T, U>
where
    T: std::ops::Mul<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    /// Creates a new Not expression
    pub fn new(lhs: Arc<U>, rhs: Arc<U>) -> Self {
        Self {
            lhs,
            rhs,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, U> RValue<T> for Mul<T, U>
where
    T: std::ops::Mul<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn eval(&self) -> Result<T, EvalError> {
        Ok(self.lhs.eval()?.mul(self.rhs.eval()?))
    }
}

/// Div is an RValue that combines two RValues with an division.
/// std::ops::Div must be defined for T.
#[derive(Debug, Clone)]
pub struct Div<T, U>
where
    T: std::ops::Div<Output = T> + std::cmp::PartialEq + Zero<T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    lhs: Arc<U>,
    rhs: Arc<U>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, U> Display for Div<T, U>
where
    T: std::ops::Div<Output = T> + std::cmp::PartialEq + Zero<T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({:?}) & ({:?})", self.lhs, self.rhs)
    }
}

impl<T, U> Div<T, U>
where
    T: std::ops::Div<Output = T> + std::cmp::PartialEq + Zero<T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    /// Creates a new Not expression
    pub fn new(lhs: Arc<U>, rhs: Arc<U>) -> Self {
        Self {
            lhs,
            rhs,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, U> RValue<T> for Div<T, U>
where
    T: std::ops::Div<Output = T> + std::cmp::PartialEq + Zero<T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    fn eval(&self) -> Result<T, EvalError> {
        let dividend = self.rhs.eval()?;
        if dividend == T::zero() {
            Err(EvalError::DivisionByZero)
        } else {
            Ok(self.lhs.eval()?.div(dividend))
        }
    }
}

/// And is an RValue that combines two RValues with a logical AND for bool and an arithmetical AND for scalars.
/// `std::ops::And` must be defined for `T`.
#[derive(Debug, Clone)]
pub struct And<T, U>
where
    T: std::ops::BitAnd<Output = T> + TypeTag + Debug + Send + Sync,
    U: RValue<T>,
{
    lhs: Arc<U>,
    rhs: Arc<U>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: std::ops::BitAnd<Output = T> + TypeTag + Debug + Send + Sync, U: RValue<T>> Display
    for And<T, U>
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({:?}) & ({:?})", self.lhs, self.rhs)
    }
}

impl<T: std::ops::BitAnd<Output = T> + TypeTag + Debug + Send + Sync, U: RValue<T>> And<T, U> {
    /// Creates a new Not expression
    pub fn new(lhs: Arc<U>, rhs: Arc<U>) -> Self {
        Self {
            lhs,
            rhs,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: std::ops::BitAnd<Output = T> + TypeTag + Debug + Send + Sync, U: RValue<T>> RValue<T>
    for And<T, U>
{
    fn eval(&self) -> Result<T, EvalError> {
        Ok(self.lhs.eval()?.bitand(self.rhs.eval()?))
    }
}

/// Or is an RValue that combines two RValues with a logical OR for bool and an arithmetical OR for scalars
/// `std::ops::Or` must be defined for `T`.
#[derive(Debug, Clone)]
pub struct Or<T: std::ops::BitOr<Output = T> + TypeTag + Debug + Send + Sync, U: RValue<T>> {
    lhs: Arc<U>,
    rhs: Arc<U>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: std::ops::BitOr<Output = T> + TypeTag + Debug + Send + Sync, U: RValue<T>> Display
    for Or<T, U>
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({:?}) & ({:?})", self.lhs, self.rhs)
    }
}

impl<T: std::ops::BitOr<Output = T> + TypeTag + Debug + Send + Sync, U: RValue<T>> Or<T, U> {
    /// Creates a new Not expression
    pub fn new(lhs: Arc<U>, rhs: Arc<U>) -> Self {
        Self {
            lhs,
            rhs,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: std::ops::BitOr<Output = T> + TypeTag + Debug + Send + Sync, U: RValue<T>> RValue<T>
    for Or<T, U>
{
    fn eval(&self) -> Result<T, EvalError> {
        Ok(self.lhs.eval()?.bitor(self.rhs.eval()?))
    }
}

/// Xor is an RValue that combines two RValues with a logical XOR for bool and an arithmetical XOR for scalars
/// /// std::ops::BitXor must be defined for T.
#[derive(Debug, Clone)]
pub struct Xor<T: std::ops::BitXor<Output = T> + TypeTag + Debug + Send + Sync, U: RValue<T>> {
    lhs: Arc<U>,
    rhs: Arc<U>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: std::ops::BitXor<Output = T> + TypeTag + Debug + Send + Sync, U: RValue<T>> Display
    for Xor<T, U>
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({:?}) ^ ({:?})", self.lhs, self.rhs)
    }
}

impl<T: std::ops::BitXor<Output = T> + TypeTag + Debug + Send + Sync, U: RValue<T>> Xor<T, U> {
    /// Creates a new Not expression
    pub fn new(lhs: Arc<U>, rhs: Arc<U>) -> Self {
        Self {
            lhs,
            rhs,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: std::ops::BitXor<Output = T> + TypeTag + Debug + Send + Sync, U: RValue<T>> RValue<T>
    for Xor<T, U>
{
    fn eval(&self) -> Result<T, EvalError> {
        Ok(self.lhs.eval()?.bitxor(self.rhs.eval()?))
    }
}
