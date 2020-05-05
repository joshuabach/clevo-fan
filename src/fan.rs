pub mod policy;
pub use policy::FanPolicy as Policy;

use crate::{
    ec::{self, ECPort},
    utils,
};
use derive_more::Display;
use std::{error::Error, fmt, num, ops::RangeInclusive, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Duty {
    ratio: f64,
}

impl fmt::Display for Duty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.ratio * 100.)?;
        if !f.alternate() {
            write!(f, "%")?;
        }

        Ok(())
    }
}

#[derive(Debug, Display)]
pub enum ParsePercentageError {
    #[display(fmt = "invalid percentage above 100%")]
    TooBig,
    #[display(fmt = "invalid percentage below 0%")]
    Negative,
    #[display(fmt = "{}", _0)]
    ParseFloat(num::ParseFloatError),
}
impl Error for ParsePercentageError {}

impl Duty {
    pub fn from_point_in_range(point: u8, range: RangeInclusive<u8>) -> Self {
        let start = *range.start() as f64;
        let end = *range.end() as f64;
        Duty {
            ratio: (point as f64 - start) / (end - start),
        }
    }

    fn to_point_in_range(&self, range: RangeInclusive<u8>) -> u8 {
        let start = *range.start() as f64;
        let end = *range.end() as f64;
        (self.ratio * (end - start) + start) as u8
    }

    pub fn from_percentage(percentage: f64) -> Result<Self, ParsePercentageError> {
        if percentage > 100. {
            Err(ParsePercentageError::TooBig)
        } else if percentage < 0. {
            Err(ParsePercentageError::Negative)
        } else {
            Ok(Duty {
                ratio: percentage / 100.,
            })
        }
    }

    pub fn as_percentage(&self) -> f64 {
        self.ratio * 100.
    }

    pub fn from_saturating_percentage(percentage: f64) -> Self {
        Self::from_percentage(percentage).unwrap_or_else(|err| match err {
            ParsePercentageError::TooBig => Self { ratio: 1. },
            ParsePercentageError::Negative => Self { ratio: 0. },
            ParsePercentageError::ParseFloat(_) => unreachable!(),
        })
    }

    pub fn from_percentage_str(percentage: &str) -> Result<Self, ParsePercentageError> {
        f64::from_str(percentage)
            .map_err(ParsePercentageError::ParseFloat)
            .and_then(Duty::from_percentage)
    }

    pub const fn min() -> Self {
        Self { ratio: 0.0 }
    }
}

#[derive(Debug)]
pub struct Speed {
    rpm: u32,
}

impl fmt::Display for Speed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.rpm)?;
        if !f.alternate() {
            write!(f, " RPM")?;
        }

        Ok(())
    }
}

impl Speed {
    pub fn from_raw_ec_bytes(lo: u8, hi: u8) -> Self {
        // See https://github.com/SkyLandTW/clevo-indicator/blob/master/src/clevo-indicator.c#L562
        const MAGIC: u32 = 2156220;
        let raw = ((hi as u16) << 8) + lo as u16;
        Speed {
            rpm: if raw > 0 {
                MAGIC / raw as u32
            } else {
                raw as u32
            },
        }
    }
}

const EC_SC_PORT_NUM: u16 = 0x66;
const EC_DATA_PORT_NUM: u16 = 0x62;
const EC_FAN_CONTROL_CMD: u8 = 0x99;
const EC_FAN_CONTROL_PORT: u8 = 0x1;
const IBF: u32 = 1;

pub struct Control {
    sc_port: ECPort,
    data_port: ECPort,
}

impl Control {
    pub fn new() -> Result<Self, utils::SyscallError> {
        unsafe {
            Ok(Control {
                sc_port: ECPort::new(EC_SC_PORT_NUM)?,
                data_port: ECPort::new(EC_DATA_PORT_NUM)?,
            })
        }
    }

    fn write(&mut self, cmd: u8, port: u8, value: u8) -> Result<(), ec::PortIOError> {
        self.sc_port.wait(IBF, 0)?;
        self.sc_port.write(cmd)?;

        self.sc_port.wait(IBF, 0)?;
        self.data_port.write(port)?;

        self.sc_port.wait(IBF, 0)?;
        self.data_port.write(value)?;

        self.sc_port.wait(IBF, 0)
    }

    pub fn set_duty(&mut self, duty: Duty) -> Result<(), ec::PortIOError> {
        self.write(
            EC_FAN_CONTROL_CMD,
            EC_FAN_CONTROL_PORT,
            duty.to_point_in_range(0..=255),
        )
    }
}
