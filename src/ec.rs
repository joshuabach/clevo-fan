use crate::{fan, utils};
use derive_more::Display;
use std::{convert::TryFrom, error::Error, fmt, io, iter, thread, time::Duration};

#[derive(Debug)]
pub struct Registers {
    pub cpu_temp: utils::Temperature,
    pub gpu_temp: utils::Temperature,
    pub fan_duty: fan::Duty,
    pub fan_speed: fan::Speed,
}

impl From<&[u8]> for Registers {
    fn from(buf: &[u8]) -> Self {
        const EC_REG_CPU_TEMP: usize = 0x07;
        const EC_REG_GPU_TEMP: usize = 0xCD;
        const EC_REG_FAN_DUTY: usize = 0xCE;
        const EC_REG_FAN_RPMS_HI: usize = 0xD0;
        const EC_REG_FAN_RPMS_LO: usize = 0xD1;

        Registers {
            cpu_temp: utils::Temperature::from_degrees_celsius(buf[EC_REG_CPU_TEMP]),
            gpu_temp: utils::Temperature::from_degrees_celsius(buf[EC_REG_GPU_TEMP]),
            fan_duty: fan::Duty::from_point_in_range(buf[EC_REG_FAN_DUTY], 0..=255),
            fan_speed: fan::Speed::from_raw_ec_bytes(
                buf[EC_REG_FAN_RPMS_LO],
                buf[EC_REG_FAN_RPMS_HI],
            ),
        }
    }
}

impl TryFrom<&mut dyn io::Read> for Registers {
    type Error = io::Error;
    fn try_from(file: &mut dyn io::Read) -> io::Result<Self> {
        const EC_REG_SIZE: usize = 0x100;

        let mut buf = [0; EC_REG_SIZE];
        file.read_exact(&mut buf)?;

        Ok(Registers::from(&buf as &[u8]))
    }
}

impl fmt::Display for Registers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "CPU Temp: {}", self.cpu_temp)?;
        writeln!(f, "GPU Temp: {}", self.gpu_temp)?;
        writeln!(f, "Fan Duty: {}", self.fan_duty)?;
        write!(f, "Fan Speed: {}", self.fan_speed)
    }
}

pub struct ECPort(cpuio::Port<u8>);

#[derive(Debug, Display)]
#[display(fmt = "Error doing Port I/O")]
pub struct PortIOError;
impl Error for PortIOError {}

impl ECPort {
    pub unsafe fn new(port: u16) -> Result<Self, utils::SyscallError> {
        nc::ioperm(port as usize, 1, 1).map_err(utils::SyscallError::from)?;
        Ok(ECPort(cpuio::Port::new(port)))
    }

    pub fn wait(&mut self, flag: u32, value: u8) -> Result<(), PortIOError> {
        const MAX_QUERIES: usize = 100;
        const QUERY_INTERVAL: Duration = Duration::from_micros(1000);

        let wait_sucessful = iter::repeat_with(|| self.0.read())
            .take(MAX_QUERIES)
            .skip_while(|data| {
                thread::sleep(QUERY_INTERVAL);
                (data >> flag) & 1 != value
            })
            .next()
            .is_some();

        if wait_sucessful {
            Ok(())
        } else {
            Err(PortIOError)
        }
    }

    pub fn write(&mut self, value: u8) -> Result<(), PortIOError> {
        self.0.write(value);
        Ok(())
    }
}
