use derive_more::{Display, From};
use std::{cmp, collections::VecDeque, error::Error, fmt, iter, ops};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Temperature {
    degrees_celsius: f64,
}

impl ops::Div<usize> for Temperature {
    type Output = Self;
    fn div(self, rhs: usize) -> Self::Output {
        Temperature {
            degrees_celsius: self.degrees_celsius / (rhs as f64),
        }
    }
}

impl iter::Sum<Temperature> for Temperature {
    fn sum<I: Iterator<Item = Temperature>>(iter: I) -> Self {
        Temperature {
            degrees_celsius: iter.map(|temp| temp.degrees_celsius).sum(),
        }
    }
}

impl fmt::Display for Temperature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1}", self.degrees_celsius)?;
        if !f.alternate() {
            write!(f, "°C")?;
        }

        Ok(())
    }
}

impl Temperature {
    pub fn from_degrees_celsius(degrees_celsius: u8) -> Self {
        Self {
            degrees_celsius: degrees_celsius as f64,
        }
    }

    pub fn as_degrees_celsius(&self) -> u8 {
        self.degrees_celsius as u8
    }

    pub const fn max() -> Self {
        Self {
            degrees_celsius: f64::MAX,
        }
    }
}

pub type FlexibleResult<T> = Result<T, Box<dyn Error>>;

pub trait ResultExt {
    fn ignore(self);
}

impl<T, E> ResultExt for Result<T, E> {
    fn ignore(self) {}
}

#[derive(Debug, Display, From)]
#[display(fmt = "Syscall error: {}", _0)]
pub struct SyscallError(nc::syscalls::Errno);
impl Error for SyscallError {}

pub struct MovingAverage<I>
where
    // Repeat type constrainst for more ergonomic error message
    I: Iterator,
    I::Item: Copy + iter::Sum<I::Item> + ops::Div<usize, Output = I::Item>,
{
    data: I,
    window_size: usize,
    buf: VecDeque<I::Item>,
}

impl<I> Iterator for MovingAverage<I>
where
    I: Iterator,
    I::Item: Copy + iter::Sum<I::Item> + ops::Div<usize, Output = I::Item>,
{
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.data.next().map(|value| {
            self.buf.push_back(value);
            if self.buf.len() > self.window_size {
                self.buf.pop_front();
            }

            let sum: I::Item = self.buf.iter().map(Clone::clone).sum();
            let avg = sum / self.buf.len();

            avg
        })
    }
}

pub trait MovingAverageIteratorExt<I> {
    fn moving_average(self, window_size: usize) -> MovingAverage<I>
    where
        I: Iterator,
        I::Item: Copy + iter::Sum<I::Item> + ops::Div<usize, Output = I::Item>;
}

impl<I> MovingAverageIteratorExt<I> for I {
    fn moving_average(self, window_size: usize) -> MovingAverage<I>
    where
        I: Iterator,
        I::Item: Copy + iter::Sum<I::Item> + ops::Div<usize, Output = I::Item>,
    {
        MovingAverage {
            data: self,
            window_size,
            buf: VecDeque::new(),
        }
    }
}

pub struct MovingMedian<I>
where
    // Repeat type constrainst for more ergonomic error message
    I: Iterator,
    I::Item: Clone + cmp::PartialOrd,
{
    data: I,
    window_size: usize,
    buf: VecDeque<I::Item>,
}

impl<I> Iterator for MovingMedian<I>
where
    I: Iterator,
    I::Item: Clone + cmp::PartialOrd,
{
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.data.next().map(|value| {
            self.buf.push_back(value);
            if self.buf.len() > self.window_size {
                self.buf.pop_front();
            }

            let mut buf = Vec::from(self.buf.clone());
            buf.sort_by(|a, b| a.partial_cmp(b).unwrap_or(cmp::Ordering::Less));
            let median = buf.remove(buf.len() / 2);

            median
        })
    }
}

pub trait MovingMedianIteratorExt<I> {
    fn moving_median(self, window_size: usize) -> MovingMedian<I>
    where
        I: Iterator,
        I::Item: Clone + cmp::PartialOrd;
}

impl<I> MovingMedianIteratorExt<I> for I {
    fn moving_median(self, window_size: usize) -> MovingMedian<I>
    where
        I: Iterator,
        I::Item: Clone + cmp::PartialOrd,
    {
        MovingMedian {
            data: self,
            window_size,
            buf: VecDeque::new(),
        }
    }
}
