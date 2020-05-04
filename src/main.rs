mod ec;
mod fan;
mod utils;

use io::Seek;
use std::{
    convert::TryFrom,
    fmt, fs,
    io::{self, Write},
    iter,
    path::PathBuf,
    thread,
    time::Duration,
};
use structopt::StructOpt;
use utils::{MovingAverageIteratorExt, MovingMedianIteratorExt, ResultExt};

type MainResult = utils::FlexibleResult<()>;

#[derive(Debug, StructOpt)]
/// Control fan of Clevo devices using the linux kernels internal interface to the EC (embeded
/// controller)
#[structopt(name = "clevo-fan")]
struct App {
    #[structopt(flatten)]
    options: Options,
    #[structopt(flatten)]
    command: Command,
}

#[derive(Debug, StructOpt)]
struct Options {
    /// SysFS path to the EC interface
    #[structopt(long, default_value = "/sys/kernel/debug/ec/ec0/io")]
    ec_path: PathBuf,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Query values from EC interface
    ///
    /// Instead of using the EC I/O ports directly, this uses the kernels representation of this in
    /// the sysfs interface, as that is likely more safe regarding concurrent write and read
    /// accesses to the same ports (e.g. due too a concurrently running `clevo-fan auto' command.
    Show {
        #[structopt(flatten)]
        values: ShowValues,
        #[structopt(flatten)]
        options: ShowOptions,
    },

    /// Set fan duty
    ///
    /// Manually set the fan duty to a specificied value.
    ///
    /// Warning: This should not be used while a `clevo-fan auto' is already running.
    Set {
        /// Desired fan duty, in percent
        #[structopt(parse(try_from_str = fan::Duty::from_percentage_str))]
        value: fan::Duty,
    },

    /// Automatically manage fan duty
    ///
    /// This periodicaly reads the core temperature from the kernels EC interface and updates the
    /// fan duty based on it. Different policies, implemented as mathematic functions are
    /// available, to determine the fan duty.
    ///
    /// Some options can be used to try to remove temporary spikes from and generally smoothen the
    /// temperature curve before calulating the fan duty based on it. This helps reduce fluctuation
    /// in the fan activity.
    ///
    /// Once the fan control loop is running, this command won't fail. Every error is handled, so
    /// that the fan never gets unattended: When failing to read the temperature, an infinitely high
    /// temperature is assumed to stay on the safe side. When the fan duty cannot be set, the cycle
    /// is skipped and setting it is tried again using the next queried temperature.  All these
    /// error conditions are reported to stderr. Any errors writing to stderr are ignored.
    Auto {
        #[structopt(flatten)]
        policies: Policies,

        /// Update interval, in milliseconds
        ///
        /// Specifies the interval length in which to poll the temperature and update the fan duty.
        #[structopt(long, short = "i", default_value = "500")]
        polling_interval: u64,

        /// Apply moving average to temperature curve
        ///
        /// Usees a moving moving average of the <moving-average> most recent temperature probes as
        /// basis to the fan duty calculation.
        ///
        /// In contrast to the moving median option, the moving average is a bit more sensitive to
        /// short temperature spikes, but can react faster to sudden, strong temperature changes.
        #[structopt(long, short = "a")]
        moving_average: Option<usize>,
        /// Apply moving median to temperature curve
        ///
        /// Uses a moving moving median of the <moving-median> most recent temperature probes as
        /// basis to the fan duty calculation.
        ///
        /// In contrast to the moving average option, the moving median is better at hiding
        /// temperature spikes, but also more sluggish in reacting to real, longer-lasting
        /// temperature surges (since they are indistinguishable from short spikes at first).
        #[structopt(long, short = "m")]
        moving_median: Option<usize>,
    },
}

#[derive(Debug, StructOpt)]
struct ShowValues {
    /// Print all available values, except gpu_temp
    #[structopt(long = "all", short = "a")]
    _all: bool,
    /// Print temperature of the CPU, in degrees Celsius
    #[structopt(long, short = "c")]
    cpu_temp: bool,
    /// Print temperature of the GPU, in degrees Celsius
    ///
    /// Warning: GPU temperature reporting via the EC is often unreliable, if it works at all.
    #[structopt(long, short = "g")]
    gpu_temp: bool,
    #[structopt(long, short = "f")]
    /// Print level of the fan, in percent
    fan_duty: bool,
    #[structopt(long, short = "r")]
    /// Print speed of the fan, in rounds per minute (RPM)
    fan_speed: bool,
}

#[derive(Debug, StructOpt)]
struct ShowOptions {
    /// Hide Labels before values
    #[structopt(long, short = "l")]
    hide_labels: bool,
    /// Hide value units
    #[structopt(long, short = "u")]
    hide_units: bool,
}

#[derive(Debug, StructOpt)]
struct Policies {
    /// Determine fan duty as a linear function of the core temperature
    ///
    /// The function looks like `duty(temp) = offset + temp * slope'. The slope and offset can be
    /// controlled via the `--linear-*' options.
    ///
    /// This is more intended as a proof-of-concept, as it is not actually a very smart policy.
    #[structopt(long,
                required_unless_one(&["exp", "square"]),
                conflicts_with_all(&["exp", "square"]))]
    linear: bool,
    /// Set slope of the fan duty function
    ///
    /// Only effective when using the linear policy.
    #[structopt(long, default_value = "1.0")]
    linear_slope: f64,
    /// Set y-axis offset of the fan duty function
    ///
    /// Only effective when using the linear policy.
    #[structopt(long, default_value = "0.0")]
    linear_offset: f64,

    /// Determine fan duty as an exponential function of the core temperature
    ///
    /// The function looks like `duty(temp) = factor * base^temp. The base and factor can be
    /// controlled via the `--exp-*' options. For the `base^temp` part, the builtin exponential
    /// functions are used, not actual exponentiation, see `--exp-base' for details.
    #[structopt(long,
                required_unless_one(&["linear", "square"]),
                conflicts_with_all(&["linear", "square"]))]
    exp: bool,
    /// Set base of the fan duty function
    ///
    /// "e" designates the natural (using `std::f64::exp') and "2" the binary exponential function
    /// (using `std::f64::exp2').
    ///
    /// Only effective when using the exponential policy.
    #[structopt(long, default_value = "e",
                possible_values(&["2", "e"]))]
    exp_base: fan::policy::ExponentialBase,
    /// Set fan duty factor for exponential function
    ///
    /// Only effective when using the exponential policy.
    #[structopt(long, default_value = "1")]
    exp_factor: f64,

    /// Determine fan duty as a quadratic function of the core temperature
    ///
    /// The function looks like this `duty(temp) = factor * temp^2'. The factor can be controlled
    /// via the `--factor' option.
    #[structopt(long,
                required_unless_one(&["linear", "exp"]),
                conflicts_with_all(&["linear", "exp"]))]
    square: bool,

    /// Set fan duty factor for square function
    ///
    /// Only effective when using the square policy.
    #[structopt(long, default_value = "0.01")]
    square_factor: f64,
}

impl App {
    fn run(self) -> MainResult {
        self.command.run(&self.options)
    }

    /// Work around shortcomings of structopt/clap
    fn post_process(mut self) -> Self {
        match self.command {
            Command::Show {
                values:
                    ShowValues {
                        _all: all,
                        gpu_temp,
                        ..
                    },
                options,
            } if all => {
                self.command = Command::Show {
                    values: ShowValues {
                        _all: true,
                        cpu_temp: true,
                        fan_duty: true,
                        fan_speed: true,
                        gpu_temp,
                    },
                    options,
                }
            }
            _ => (),
        }

        self
    }
}

impl Command {
    fn run(self, general_options: &Options) -> MainResult {
        match self {
            Command::Show { values, options } => {
                let mut ec = fs::OpenOptions::new()
                    .read(true)
                    .open(&general_options.ec_path)?;
                let ec = ec::Registers::try_from(&mut ec as &mut dyn io::Read)?;

                let values: [(_, &dyn fmt::Display, _); 4] = [
                    (values.cpu_temp, &ec.cpu_temp, "CPU Temp"),
                    (values.gpu_temp, &ec.gpu_temp, "GPU Temp"),
                    (values.fan_duty, &ec.fan_duty, "Fan Duty"),
                    (values.fan_speed, &ec.fan_speed, "Fan Speed"),
                ];
                for (should_print, value, label) in values.iter() {
                    if *should_print {
                        if !options.hide_labels {
                            write!(io::stdout(), "{}: ", label)?;
                        }
                        if options.hide_units {
                            writeln!(io::stdout(), "{:#}", value)?;
                        } else {
                            writeln!(io::stdout(), "{}", value)?;
                        }
                    }
                }

                if values.iter().all(|(should_print, _, _)| !should_print) {
                    writeln!(
                        io::stderr(),
                        "Warning: No values are being printed, you might want to use `-a'. See `--help' for further information."
                    )?;
                }
            }
            Command::Set { value } => {
                if value < fan::Duty::from_percentage(37.).unwrap() {
                    writeln!(
                        io::stderr(),
                        "Warning: Fan only becomse active from 38% duty upwards. Setting duty below this will disable the fan entirely."
                    )?;
                }

                fan::Control::new()?.set_duty(value)?
            }
            Command::Auto {
                policies,
                polling_interval,
                moving_average: moving_average_backlog,
                moving_median: moving_median_backlog,
            } => {
                let mut ec = fs::OpenOptions::new()
                    .read(true)
                    .open(&general_options.ec_path)?;

                let policy: Box<dyn fan::Policy<Input = utils::Temperature>> = if policies.linear {
                    Box::new(fan::policy::Linear {
                        slope: policies.linear_slope,
                        offset: policies.linear_offset,
                    })
                } else if policies.exp {
                    Box::new(fan::policy::Exponential {
                        base: policies.exp_base,
                        factor: policies.exp_factor,
                    })
                } else if policies.square {
                    Box::new(fan::policy::Quadratic {
                        factor: policies.square_factor,
                    })
                } else {
                    unreachable!("This should be handled by structopt")
                };

                let mut fan = fan::Control::new()?;

                // Infinite iterator. All errors are handeled, this will /never/ fail.
                let temp_curve = iter::repeat_with(|| {
                    ec.seek(io::SeekFrom::Start(0))?;
                    let ec = ec::Registers::try_from(&mut ec as &mut dyn io::Read)?;
                    Ok(ec.cpu_temp)
                })
                .map(|res: Result<_, io::Error>| {
                    res.unwrap_or_else(|err| {
                        writeln!(
                            io::stderr(),
                            "Error: Cannot read temperature: {}, assuming the worst",
                            err
                        )
                        .ignore();
                        utils::Temperature::max()
                    })
                });

                let normalized_temp_curve: Box<dyn Iterator<Item = utils::Temperature>> =
                    if let Some(backlog) = moving_median_backlog {
                        Box::new(temp_curve.moving_median(backlog))
                    } else if let Some(backlog) = moving_average_backlog {
                        Box::new(temp_curve.moving_average(backlog))
                    } else {
                        Box::new(temp_curve)
                    };

                normalized_temp_curve
                    .map(|temp| policy.next_fan_duty(temp))
                    .for_each(|duty| {
                        fan.set_duty(duty).unwrap_or_else(|err| {
                            writeln!(io::stderr(), "Error: Cannot set fan duty: {}", err).ignore()
                        });

                        thread::sleep(Duration::from_millis(polling_interval));
                    });
            }
        }

        Ok(())
    }
}

fn main() -> MainResult {
    App::from_args().post_process().run()
}
