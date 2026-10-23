//! Desktop-independent compatibility values and synchronization fingerprints.
#![forbid(unsafe_code)]

pub mod canonical;
pub mod csv;
pub mod domain;

pub use domain::*;
pub use rust_decimal::Decimal;
