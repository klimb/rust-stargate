

//! Set of functions related to time handling

use jiff::Zoned;
use jiff::fmt::StdIoWrite;
use jiff::fmt::strtime::{BrokenDownTime, Config};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{SGResult, SGSimpleError};
use crate::show_error;

/// Format the given date according to this time format style.
fn format_zoned<W: Write>(out: &mut W, zoned: Zoned, fmt: &str) -> SGResult<()> {
    let tm = BrokenDownTime::from(&zoned);
    let mut out = StdIoWrite(out);
    let config = Config::new().lenient(true);
    tm.format_with_config(&config, fmt, &mut out)
        .map_err(|x| SGSimpleError::new(1, x.to_string()))
}

/// Convert a SystemTime` to a number of seconds since UNIX_EPOCH
pub fn system_time_to_sec(time: SystemTime) -> (i64, u32) {
    if time > UNIX_EPOCH {
        let d = time.duration_since(UNIX_EPOCH).unwrap();
        (d.as_secs() as i64, d.subsec_nanos())
    } else {
        let d = UNIX_EPOCH.duration_since(time).unwrap();
        (-(d.as_secs() as i64), d.subsec_nanos())
    }
}

pub mod format {
    pub static FULL_ISO: &str = "%Y-%m-%d %H:%M:%S.%N %z";
    pub static LONG_ISO: &str = "%Y-%m-%d %H:%M";
    pub static ISO: &str = "%Y-%m-%d";
}

/// Sets how `format_system_time` behaves if the time cannot be converted.
pub enum FormatSystemTimeFallback {
    Integer,
    IntegerError,
    Float,
}

/// Format a `SystemTime` according to given fmt, and append to vector out.
pub fn format_system_time<W: Write>(
    out: &mut W,
    time: SystemTime,
    fmt: &str,
    mode: FormatSystemTimeFallback
) -> SGResult<()> {
    let zoned: Result<Zoned, _> = time.try_into();
    match zoned {
        Ok(zoned) => format_zoned(out, zoned, fmt),
        Err(_) => {
            let (mut secs, mut nsecs) = system_time_to_sec(time);
            match mode {
                FormatSystemTimeFallback::Integer => out.write_all(secs.to_string().as_bytes())?,
                FormatSystemTimeFallback::IntegerError => {
                    let str = secs.to_string();
                    show_error!("time '{str}' is out of range");
                    out.write_all(str.as_bytes())?;
                }
                FormatSystemTimeFallback::Float => {
                    if secs < 0 && nsecs != 0 {
                        secs -= 1;
                        nsecs = 1_000_000_000 - nsecs;
                    }
                    out.write_fmt(format_args!("{secs}.{nsecs:09}"))?;
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::time::{FormatSystemTimeFallback, format_system_time};
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn test_simple_system_time() {
        unsafe { std::env::set_var("TZ", "UTC0") };

        let time = UNIX_EPOCH;
        let mut out = Vec::new();
        format_system_time(
            &mut out,
            time,
            "%Y-%m-%d %H:%M",
            FormatSystemTimeFallback::Integer
        )
        .expect("Formatting error.");
        assert_eq!(String::from_utf8(out).unwrap(), "1970-01-01 00:00");

        let mut out = Vec::new();
        format_system_time(
            &mut out,
            time,
            "%Y-%m-%d %H:%M:%S.%N %z",
            FormatSystemTimeFallback::Integer
        )
        .expect("Formatting error.");
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "1970-01-01 00:00:00.000000000 +0000"
        );
    }

    #[test]
    fn test_large_system_time() {
        let time = UNIX_EPOCH + Duration::from_secs(67_768_036_191_763_200);
        let mut out = Vec::new();
        format_system_time(
            &mut out,
            time,
            "%Y-%m-%d %H:%M",
            FormatSystemTimeFallback::Integer
        )
        .expect("Formatting error.");
        assert_eq!(String::from_utf8(out).unwrap(), "67768036191763200");

        let time = UNIX_EPOCH - Duration::from_secs(67_768_040_922_076_800);
        let mut out = Vec::new();
        format_system_time(
            &mut out,
            time,
            "%Y-%m-%d %H:%M",
            FormatSystemTimeFallback::Integer
        )
        .expect("Formatting error.");
        assert_eq!(String::from_utf8(out).unwrap(), "-67768040922076800");
    }

    #[test]
    fn test_large_system_time_float() {
        let time =
            UNIX_EPOCH + Duration::from_secs(67_768_036_191_763_000) + Duration::from_nanos(123);
        let mut out = Vec::new();
        format_system_time(
            &mut out,
            time,
            "%Y-%m-%d %H:%M",
            FormatSystemTimeFallback::Float
        )
        .expect("Formatting error.");
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "67768036191763000.000000123"
        );

        let time =
            UNIX_EPOCH - Duration::from_secs(67_768_040_922_076_000) + Duration::from_nanos(123);
        let mut out = Vec::new();
        format_system_time(
            &mut out,
            time,
            "%Y-%m-%d %H:%M",
            FormatSystemTimeFallback::Float
        )
        .expect("Formatting error.");
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "-67768040922076000.000000123"
        );
    }
}

