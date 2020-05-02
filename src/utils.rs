use derive_more::{Display, From};
use std::{error::Error, fmt};

#[derive(Debug)]
pub struct Temperature {
    degrees_celsius: u8,
}

impl fmt::Display for Temperature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.degrees_celsius)?;
        if !f.alternate() {
            write!(f, "°C")?;
        }

        Ok(())
    }
}

impl Temperature {
    pub fn from_degrees_celsius(degrees_celsius: u8) -> Self {
        Self { degrees_celsius }
    }
}

pub type FlexibleResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug, Display, From)]
#[display(fmt = "Syscall error: {}", _0)]
pub struct SyscallError(nc::syscalls::Errno);
impl Error for SyscallError {}
