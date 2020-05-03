use crate::utils;
use derive_more::Display;
use std::{error::Error, str::FromStr};

pub trait FanPolicy {
    type Input;
    fn next_fan_duty(&self, input: Self::Input) -> super::Duty;
}

pub struct Linear {
    pub slope: f64,
    pub offset: f64,
}

impl FanPolicy for Linear {
    type Input = utils::Temperature;
    fn next_fan_duty(&self, temp: Self::Input) -> super::Duty {
        super::Duty::from_saturating_percentage(
            self.offset + temp.as_degrees_celsius() as f64 * self.slope,
        )
    }
}

impl Default for Linear {
    fn default() -> Self {
        Linear {
            slope: 1.0,
            offset: 0.0,
        }
    }
}

pub struct Exponential {
    pub base: ExponentialBase,
    pub factor: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum ExponentialBase {
    Euler,
    Binary,
}

#[derive(Debug, Display)]
#[display(fmt = "Invalid exponential base: {}" _0)]
pub struct InvalidExponentialBase(String);
impl Error for InvalidExponentialBase {}
impl FromStr for ExponentialBase {
    type Err = InvalidExponentialBase;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use self::ExponentialBase::*;
        match s {
            "e" | "euler" => Ok(Euler),
            "2" | "bin" | "binary" => Ok(Binary),
            _ => Err(InvalidExponentialBase(s.to_owned())),
        }
    }
}

impl ExponentialBase {
    fn exp(self, exponent: f64) -> f64 {
        use self::ExponentialBase::*;
        match self {
            Euler => exponent.exp(),
            Binary => exponent.exp2(),
        }
    }
}

impl FanPolicy for Exponential {
    type Input = utils::Temperature;
    fn next_fan_duty(&self, temp: Self::Input) -> super::Duty {
        super::Duty::from_saturating_percentage(
            self.factor * self.base.exp(temp.as_degrees_celsius() as f64),
        )
    }
}

pub struct Quadratic {
    pub factor: f64,
}

impl FanPolicy for Quadratic {
    type Input = utils::Temperature;
    fn next_fan_duty(&self, temp: Self::Input) -> super::Duty {
        super::Duty::from_saturating_percentage(
            self.factor * (temp.as_degrees_celsius() as f64).powi(2),
        )
    }
}
