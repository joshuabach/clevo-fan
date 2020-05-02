mod ec;
mod fan;
mod utils;

use std::{
    convert::TryFrom,
    fmt, fs,
    io::{self, Write},
    path::PathBuf,
};
use structopt::StructOpt;

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
        }

        Ok(())
    }
}

fn main() -> MainResult {
    App::from_args().post_process().run()
}
