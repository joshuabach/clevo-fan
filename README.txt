clevo-fan - Automatically manage fan duty

Inspired by: https://github.com/SkyLandTW/clevo-indicator

You probably need to `modprobe ec_sys` before running.

This periodicaly reads the core temperature from the kernels EC interface and updates the
fan duty based on it.  Different policies, implemented as mathematic functions are
available, to determine the fan duty.

Some options can be used to try to remove temporary spikes from and generally smoothen the
temperature before calulating the fan duty based on it. This helps reduce fluctuation in
the fan activity. Some options also directly affect the fan curve.

Once the fan control loop is running, this command won't fail. Every error is handled, so
that the fan never gets unattended: When failing to read the temperature, an infinitely
high temperature is assumed to stay on the safe side.  When the fan duty cannot be set,
the cycle is skipped and setting it is tried again using the next queried temperature.
All these error conditions are reported to stderr. Any errors writing to stderr (or to
stdout) are ignored.

USAGE:
    clevo-fan auto [FLAGS] [OPTIONS] --exp --linear --square

FLAGS:
        --exp        Determine fan duty as an exponential function of the core temperature
    -h, --help       Prints help information
        --linear     Determine fan duty as a linear function of the core temperature
        --monitor    Monitor temperature and fan duty curves
        --square     Determine fan duty as a quadratic function of the core temperature
    -V, --version    Prints version information

OPTIONS:
        --exp-base <exp-base>
            Set base of the fan duty function [default: e]  [possible values: 2, e]
        --exp-factor <exp-factor>
            Set fan duty factor for exponential function [default: 1]
        --linear-offset <linear-offset>
            Set y-axis offset of the fan duty function [default: 0.0]
        --linear-slope <linear-slope>
            Set slope of the fan duty function [default: 1.0]
        --max-unchanged-cycles <max-unchanged-cycles>
            Maximum number of consequtive fan duty changes to ignore [default: 10]
        --min-fan-change <min-fan-change>
            Only apply fan duty changes smaller than this value [default: 0.0]

    -a, --moving-average <moving-average>
            Apply moving average to temperature curve
    -m, --moving-median <moving-median>
            Apply moving median to temperature curve
    -i, --polling-interval <polling-interval>
            Update interval, in milliseconds [default: 500]
        --square-factor <square-factor>
            Set fan duty factor for square function [default: 0.01]
