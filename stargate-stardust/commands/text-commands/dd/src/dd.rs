

mod blocks;
mod bufferedoutput;
mod conversion_tables;
mod datastructures;
mod numbers;
mod parseargs;
mod progress;

use crate::bufferedoutput::BufferedOutput;
use blocks::conv_block_unblock_helper;
use datastructures::*;
#[cfg(any(target_os = "linux"))]
use nix::fcntl::FcntlArg::F_SETFL;
#[cfg(any(target_os = "linux"))]
use nix::fcntl::OFlag;
use parseargs::Parser;
use progress::ProgUpdateType;
use progress::{ProgUpdate, ReadStat, StatusLevel, WriteStat, gen_prog_updater};
use sgcore::io::OwnedFileDescriptorOrHandle;
use sgcore::translate;

use std::cmp;
use std::env;
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Stdout, Write};
#[cfg(any(target_os = "linux"))]
use std::os::fd::AsFd;
#[cfg(any(target_os = "linux"))]
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::{
    fs::FileTypeExt,
    io::{AsRawFd, FromRawFd},
};

use std::path::Path;
use std::sync::atomic::AtomicU8;
use std::sync::{Arc, atomic::Ordering::Relaxed, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use clap::{Arg, Command};
use gcd::Gcd;
#[cfg(target_os = "linux")]
use nix::{
    errno::Errno,
    fcntl::{PosixFadviseAdvice, posix_fadvise},
};
use sgcore::display::Quotable;
use sgcore::error::{FromIo, SGResult};
use sgcore::error::{SGSimpleError, set_exit_code};
#[cfg(target_os = "linux")]
use sgcore::show_if_err;
use sgcore::{format_usage, show_error};

const BUF_INIT_BYTE: u8 = 0xDD;

/// Final settings after parsing
#[derive(Default)]
struct Settings {
    infile: Option<String>,
    outfile: Option<String>,
    ibs: usize,
    obs: usize,
    skip: u64,
    seek: u64,
    count: Option<Num>,
    iconv: IConvFlags,
    iflags: IFlags,
    oconv: OConvFlags,
    oflags: OFlags,
    status: Option<StatusLevel>,
    /// Whether the output writer should buffer partial blocks until complete.
    buffered: bool,
}

/// A timer which triggers on a given interval
///
/// After being constructed with [`Alarm::with_interval`], [`Alarm::get_trigger`]
/// will return [`ALARM_TRIGGER_TIMER`] once per the given [`Duration`].
/// Alarm can be manually triggered with closure returned by [`Alarm::manual_trigger_fn`].
/// [`Alarm::get_trigger`] will return [`ALARM_TRIGGER_SIGNAL`] in this case.
///
/// Can be cloned, but the trigger status is shared across all instances so only
/// the first caller each interval will yield true.
///
/// When all instances are dropped the background thread will exit on the next interval.
pub struct Alarm {
    interval: Duration,
    trigger: Arc<AtomicU8>,
}

pub const ALARM_TRIGGER_NONE: u8 = 0;
pub const ALARM_TRIGGER_TIMER: u8 = 1;
pub const ALARM_TRIGGER_SIGNAL: u8 = 2;

impl Alarm {
    /// use to construct alarm timer with duration
    pub fn with_interval(interval: Duration) -> Self {
        let trigger = Arc::new(AtomicU8::default());

        let weak_trigger = Arc::downgrade(&trigger);
        thread::spawn(move || {
            while let Some(trigger) = weak_trigger.upgrade() {
                thread::sleep(interval);
                trigger.store(ALARM_TRIGGER_TIMER, Relaxed);
            }
        });

        Self { interval, trigger }
    }

    /// Returns a closure that allows to manually trigger the alarm
    ///
    /// This is useful for cases where more than one alarm even source exists
    /// In case of `dd` there is the SIGUSR1/SIGINFO case where we want to
    /// trigger an manual progress report.
    pub fn manual_trigger_fn(&self) -> Box<dyn Send + Sync + Fn()> {
        let weak_trigger = Arc::downgrade(&self.trigger);
        Box::new(move || {
            if let Some(trigger) = weak_trigger.upgrade() {
                trigger.store(ALARM_TRIGGER_SIGNAL, Relaxed);
            }
        })
    }

    /// Use this function to poll for any pending alarm event
    ///
    /// Returns `ALARM_TRIGGER_NONE` for no pending event.
    /// Returns `ALARM_TRIGGER_TIMER` if the event was triggered by timer
    /// Returns `ALARM_TRIGGER_SIGNAL` if the event was triggered manually
    /// by the closure returned from `manual_trigger_fn`
    pub fn get_trigger(&self) -> u8 {
        self.trigger.swap(ALARM_TRIGGER_NONE, Relaxed)
    }

    pub fn get_interval(&self) -> Duration {
        self.interval
    }
}

/// A number in blocks or bytes
///
/// Some values (seek, skip, iseek, oseek) can have values either in blocks or in bytes.
/// We need to remember this because the size of the blocks (ibs) is only known after parsing
/// all the arguments.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Num {
    Blocks(u64),
    Bytes(u64),
}

impl Default for Num {
    fn default() -> Self {
        Self::Blocks(0)
    }
}

impl Num {
    fn force_bytes_if(self, force: bool) -> Self {
        match self {
            Self::Blocks(n) if force => Self::Bytes(n),
            count => count,
        }
    }

    fn to_bytes(self, block_size: u64) -> u64 {
        match self {
            Self::Blocks(n) => n * block_size,
            Self::Bytes(n) => n,
        }
    }
}

/// Data sources.
///
/// Use [`Source::stdin_as_file`] if available to enable more
/// fine-grained access to reading from stdin.
enum Source {
    /// Input from stdin.
    #[cfg(not(unix))]
    Stdin(io::Stdin),

    /// Input from a file.
    File(File),

    /// Input from stdin, opened from its file descriptor.
    StdinFile(File),

    /// Input from a named pipe, also known as a FIFO.
    Fifo(File),
}

impl Source {
    /// Create a source from stdin using its raw file descriptor.
    ///
    /// This returns an instance of the `Source::StdinFile` variant,
    /// using the raw file descriptor of [`std::io::Stdin`] to create
    /// the [`std::fs::File`] parameter. You can use this instead of
    /// `Source::Stdin` to allow reading from stdin without consuming
    /// the entire contents of stdin when this process terminates.
    fn stdin_as_file() -> Self {
        let fd = io::stdin().as_raw_fd();
        let f = unsafe { File::from_raw_fd(fd) };
        Self::StdinFile(f)
    }

    /// The length of the data source in number of bytes.
    ///
    /// If it cannot be determined, then this function returns 0.
    fn len(&self) -> io::Result<i64> {
        #[allow(clippy::match_wildcard_for_single_variants)]
        match self {
            Self::File(f) => Ok(f.metadata()?.len().try_into().unwrap_or(i64::MAX)),
            _ => Ok(0),
        }
    }

    fn skip(&mut self, n: u64) -> io::Result<u64> {
        match self {
            #[cfg(not(unix))]
            Self::Stdin(stdin) => match io::copy(&mut stdin.take(n), &mut io::sink()) {
                Ok(m) if m < n => {
                    show_error!(
                        "{}",
                        translate!("dd-error-cannot-skip-offset", "file" => "standard input")
                    );
                    Ok(m)
                }
                Ok(m) => Ok(m),
                Err(e) => Err(e),
            },
            Self::StdinFile(f) => {
                if let Ok(Some(len)) = try_get_len_of_block_device(f) {
                    if len < n {
                        show_error!(
                            "{}",
                            translate!("dd-error-cannot-skip-invalid", "file" => "standard input")
                        );
                        set_exit_code(1);
                        return Ok(len);
                    }
                }
                match io::copy(&mut f.take(n), &mut io::sink()) {
                    Ok(m) if m < n => {
                        show_error!(
                            "{}",
                            translate!("dd-error-cannot-skip-offset", "file" => "standard input")
                        );
                        Ok(m)
                    }
                    Ok(m) => Ok(m),
                    Err(e) => Err(e),
                }
            }
            Self::File(f) => f.seek(SeekFrom::Current(n.try_into().unwrap())),
            Self::Fifo(f) => io::copy(&mut f.take(n), &mut io::sink()),
        }
    }

    /// Discard the system file cache for the given portion of the data source.
    ///
    /// `offset` and `len` specify a contiguous portion of the data
    /// source. This function informs the kernel that the specified
    /// portion of the source is no longer needed. If not possible,
    /// then this function returns an error.
    #[cfg(target_os = "linux")]
    fn discard_cache(&self, offset: libc::off_t, len: libc::off_t) -> nix::Result<()> {
        #[allow(clippy::match_wildcard_for_single_variants)]
        match self {
            Self::File(f) => {
                let advice = PosixFadviseAdvice::POSIX_FADV_DONTNEED;
                posix_fadvise(f.as_fd(), offset, len, advice)
            }
            _ => Err(Errno::ESPIPE),
        }
    }
}

impl Read for Source {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            #[cfg(not(unix))]
            Self::Stdin(stdin) => stdin.read(buf),
            Self::File(f) => f.read(buf),
            Self::StdinFile(f) => f.read(buf),
            Self::Fifo(f) => f.read(buf),
        }
    }
}

/// The source of the data, configured with the given settings.
///
/// Use the [`Input::new_stdin`] or [`Input::new_file`] functions to
/// construct a new instance of this struct. Then pass the instance to
/// the [`dd_copy`] function to execute the main copy operation
/// for `dd`.
struct Input<'a> {
    /// The source from which bytes will be read.
    src: Source,

    /// Configuration settings for how to read the data.
    settings: &'a Settings,
}

impl<'a> Input<'a> {
    /// Instantiate this struct with stdin as a source.
    fn new_stdin(settings: &'a Settings) -> SGResult<Self> {
        let mut src = Source::stdin_as_file();
        if let Source::StdinFile(f) = &src {
            if settings.iflags.directory && !f.metadata()?.is_dir() {
                return Err(SGSimpleError::new(
                    1,
                    translate!("dd-error-not-directory", "file" => "standard input")
                ));
            }
        }
        if settings.skip > 0 {
            src.skip(settings.skip)?;
        }
        Ok(Self { src, settings })
    }

    /// Instantiate this struct with the named file as a source.
    fn new_file(filename: &Path, settings: &'a Settings) -> SGResult<Self> {
        let src = {
            let mut opts = OpenOptions::new();
            opts.read(true);

            #[cfg(any(target_os = "linux"))]
            if let Some(libc_flags) = make_linux_iflags(&settings.iflags) {
                opts.custom_flags(libc_flags);
            }

            opts.open(filename).map_err_context(
                || translate!("dd-error-failed-to-open", "path" => filename.quote())
            )?
        };

        let mut src = Source::File(src);
        if settings.skip > 0 {
            src.skip(settings.skip)?;
        }
        Ok(Self { src, settings })
    }

    /// Instantiate this struct with the named pipe as a source.
    fn new_fifo(filename: &Path, settings: &'a Settings) -> SGResult<Self> {
        let mut opts = OpenOptions::new();
        opts.read(true);
        #[cfg(any(target_os = "linux"))]
        opts.custom_flags(make_linux_iflags(&settings.iflags).unwrap_or(0));
        let mut src = Source::Fifo(opts.open(filename)?);
        if settings.skip > 0 {
            src.skip(settings.skip)?;
        }
        Ok(Self { src, settings })
    }
}

#[cfg(any(target_os = "linux"))]
fn make_linux_iflags(iflags: &IFlags) -> Option<libc::c_int> {
    let mut flag = 0;

    if iflags.direct {
        flag |= libc::O_DIRECT;
    }
    if iflags.directory {
        flag |= libc::O_DIRECTORY;
    }
    if iflags.dsync {
        flag |= libc::O_DSYNC;
    }
    if iflags.noatime {
        flag |= libc::O_NOATIME;
    }
    if iflags.noctty {
        flag |= libc::O_NOCTTY;
    }
    if iflags.nofollow {
        flag |= libc::O_NOFOLLOW;
    }
    if iflags.nonblock {
        flag |= libc::O_NONBLOCK;
    }
    if iflags.sync {
        flag |= libc::O_SYNC;
    }

    if flag == 0 { None } else { Some(flag) }
}

impl Read for Input<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut base_idx = 0;
        let target_len = buf.len();
        loop {
            match self.src.read(&mut buf[base_idx..]) {
                Ok(0) => return Ok(base_idx),
                Ok(rlen) if self.settings.iflags.fullblock => {
                    base_idx += rlen;

                    if base_idx >= target_len {
                        return Ok(target_len);
                    }
                }
                Ok(len) => return Ok(len),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => (),
                Err(_) if self.settings.iconv.noerror => return Ok(base_idx),
                Err(e) => return Err(e),
            }
        }
    }
}

impl Input<'_> {
    /// Discard the system file cache for the given portion of the input.
    ///
    /// `offset` and `len` specify a contiguous portion of the input.
    /// This function informs the kernel that the specified portion of
    /// the input file is no longer needed. If not possible, then this
    /// function prints an error message to stderr and sets the exit
    /// status code to 1.
    #[cfg_attr(not(target_os = "linux"), allow(clippy::unused_self, unused_variables))]
    fn discard_cache(&self, offset: libc::off_t, len: libc::off_t) {
        #[cfg(target_os = "linux")]
        {
            show_if_err!(
                self.src
                    .discard_cache(offset, len)
                    .map_err_context(|| translate!("dd-error-failed-discard-cache-input"))
            );
        }
        #[cfg(not(target_os = "linux"))]
        {
        }
    }

    /// Fills a given buffer.
    /// Reads in increments of 'self.ibs'.
    /// The start of each ibs-sized read follows the previous one.
    fn fill_consecutive(&mut self, buf: &mut Vec<u8>) -> io::Result<ReadStat> {
        let mut reads_complete = 0;
        let mut reads_partial = 0;
        let mut bytes_total = 0;

        for chunk in buf.chunks_mut(self.settings.ibs) {
            match self.read(chunk)? {
                rlen if rlen == self.settings.ibs => {
                    bytes_total += rlen;
                    reads_complete += 1;
                }
                rlen if rlen > 0 => {
                    bytes_total += rlen;
                    reads_partial += 1;
                }
                _ => break,
            }
        }
        buf.truncate(bytes_total);
        Ok(ReadStat {
            reads_complete,
            reads_partial,
            records_truncated: 0,
            bytes_total: bytes_total.try_into().unwrap(),
        })
    }

    /// Fills a given buffer.
    /// Reads in increments of 'self.ibs'.
    /// The start of each ibs-sized read is aligned to multiples of ibs; remaining space is filled with the 'pad' byte.
    fn fill_blocks(&mut self, buf: &mut Vec<u8>, pad: u8) -> io::Result<ReadStat> {
        let mut reads_complete = 0;
        let mut reads_partial = 0;
        let mut base_idx = 0;
        let mut bytes_total = 0;

        while base_idx < buf.len() {
            let next_blk = cmp::min(base_idx + self.settings.ibs, buf.len());
            let target_len = next_blk - base_idx;

            match self.read(&mut buf[base_idx..next_blk])? {
                0 => break,
                rlen if rlen < target_len => {
                    bytes_total += rlen;
                    reads_partial += 1;
                    let padding = vec![pad; target_len - rlen];
                    buf.splice(base_idx + rlen..next_blk, padding.into_iter());
                }
                rlen => {
                    bytes_total += rlen;
                    reads_complete += 1;
                }
            }

            base_idx += self.settings.ibs;
        }

        buf.truncate(base_idx);
        Ok(ReadStat {
            reads_complete,
            reads_partial,
            records_truncated: 0,
            bytes_total: bytes_total.try_into().unwrap(),
        })
    }
}

enum Density {
    Sparse,
    Dense,
}

/// Data destinations.
enum Dest {
    /// Output to stdout.
    Stdout(Stdout),

    /// Output to a file.
    ///
    /// The [`Density`] component indicates whether to attempt to
    /// write a sparse file when all-zero blocks are encountered.
    File(File, Density),

    /// Output to a named pipe, also known as a FIFO.
    Fifo(File),

    /// Output to nothing, dropping each byte written to the output.
    Sink,
}

impl Dest {
    fn fsync(&mut self) -> io::Result<()> {
        match self {
            Self::Stdout(stdout) => stdout.flush(),
            Self::File(f, _) => {
                f.flush()?;
                f.sync_all()
            }
            Self::Fifo(f) => {
                f.flush()?;
                f.sync_all()
            }
            Self::Sink => Ok(()),
        }
    }

    fn fdatasync(&mut self) -> io::Result<()> {
        match self {
            Self::Stdout(stdout) => stdout.flush(),
            Self::File(f, _) => {
                f.flush()?;
                f.sync_data()
            }
            Self::Fifo(f) => {
                f.flush()?;
                f.sync_data()
            }
            Self::Sink => Ok(()),
        }
    }

    fn seek(&mut self, n: u64) -> io::Result<u64> {
        match self {
            Self::Stdout(stdout) => io::copy(&mut io::repeat(0).take(n), stdout),
            Self::File(f, _) => {
                if let Ok(Some(len)) = try_get_len_of_block_device(f) {
                    if len < n {
                        show_error!(
                            "{}",
                            translate!("dd-error-cannot-seek-invalid", "output" => "standard output")
                        );
                        set_exit_code(1);
                        return Ok(len);
                    }
                }
                f.seek(SeekFrom::Current(n.try_into().unwrap()))
            }
            Self::Fifo(f) => {
                io::copy(&mut f.take(n), &mut io::sink())
            }
            Self::Sink => Ok(0),
        }
    }

    /// Truncate the underlying file to the current stream position, if possible.
    fn truncate(&mut self) -> io::Result<()> {
        #[allow(clippy::match_wildcard_for_single_variants)]
        match self {
            Self::File(f, _) => {
                let pos = f.stream_position()?;
                f.set_len(pos)
            }
            _ => Ok(()),
        }
    }

    /// Discard the system file cache for the given portion of the destination.
    ///
    /// `offset` and `len` specify a contiguous portion of the
    /// destination. This function informs the kernel that the
    /// specified portion of the destination is no longer needed. If
    /// not possible, then this function returns an error.
    #[cfg(target_os = "linux")]
    fn discard_cache(&self, offset: libc::off_t, len: libc::off_t) -> nix::Result<()> {
        match self {
            Self::File(f, _) => {
                let advice = PosixFadviseAdvice::POSIX_FADV_DONTNEED;
                posix_fadvise(f.as_fd(), offset, len, advice)
            }
            _ => Err(Errno::ESPIPE),
        }
    }

    /// The length of the data destination in number of bytes.
    ///
    /// If it cannot be determined, then this function returns 0.
    fn len(&self) -> io::Result<i64> {
        #[allow(clippy::match_wildcard_for_single_variants)]
        match self {
            Self::File(f, _) => Ok(f.metadata()?.len().try_into().unwrap_or(i64::MAX)),
            _ => Ok(0),
        }
    }
}

/// Decide whether the given buffer is all zeros.
fn is_sparse(buf: &[u8]) -> bool {
    buf.iter().all(|&e| e == 0u8)
}

/// Handle O_DIRECT write errors by temporarily removing the flag and retrying.
/// This follows GNU dd behavior for partial block writes with O_DIRECT.
#[cfg(any(target_os = "linux"))]
fn handle_o_direct_write(f: &mut File, buf: &[u8], original_error: io::Error) -> io::Result<usize> {
    use nix::fcntl::{FcntlArg, OFlag, fcntl};

    let oflags = match fcntl(&mut *f, FcntlArg::F_GETFL) {
        Ok(flags) => OFlag::from_bits_retain(flags),
        Err(_) => return Err(original_error),
    };

    if oflags.contains(OFlag::O_DIRECT) {
        let flags_without_direct = oflags - OFlag::O_DIRECT;

        if fcntl(&mut *f, FcntlArg::F_SETFL(flags_without_direct)).is_err() {
            return Err(original_error);
        }

        let write_result = f.write(buf);

        if let Err(os_err) = fcntl(&mut *f, FcntlArg::F_SETFL(oflags)) {
            show_error!("Failed to restore O_DIRECT flag: {}", os_err);
        }

        write_result
    } else {
        Err(original_error)
    }
}

/// Stub for non-Linux platforms - just return the original error.
#[cfg(not(any(target_os = "linux")))]
fn handle_o_direct_write(
    _f: &mut File,
    _buf: &[u8],
    original_error: io::Error
) -> io::Result<usize> {
    Err(original_error)
}

impl Write for Dest {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::File(f, Density::Sparse) if is_sparse(buf) => {
                let seek_amt: i64 = buf
                    .len()
                    .try_into()
                    .expect("Internal dd Error: Seek amount greater than signed 64-bit integer");
                f.seek(SeekFrom::Current(seek_amt))?;
                Ok(buf.len())
            }
            Self::File(f, _) => {
                match f.write(buf) {
                    Ok(len) => Ok(len),
                    Err(e)
                        if e.kind() == io::ErrorKind::InvalidInput
                            && e.raw_os_error() == Some(libc::EINVAL) =>
                    {
                        handle_o_direct_write(f, buf, e)
                    }
                    Err(e) => Err(e),
                }
            }
            Self::Stdout(stdout) => stdout.write(buf),
            Self::Fifo(f) => f.write(buf),
            Self::Sink => Ok(buf.len()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Stdout(stdout) => stdout.flush(),
            Self::File(f, _) => f.flush(),
            Self::Fifo(f) => f.flush(),
            Self::Sink => Ok(()),
        }
    }
}

/// The destination of the data, configured with the given settings.
///
/// Use the [`Output::new_stdout`] or [`Output::new_file`] functions
/// to construct a new instance of this struct. Then use the
/// [`dd_copy`] function to execute the main copy operation for
/// `dd`.
struct Output<'a> {
    /// The destination to which bytes will be written.
    dst: Dest,

    /// Configuration settings for how to read and write the data.
    settings: &'a Settings,
}

impl<'a> Output<'a> {
    /// Instantiate this struct with stdout as a destination.
    fn new_stdout(settings: &'a Settings) -> SGResult<Self> {
        let mut dst = Dest::Stdout(io::stdout());
        dst.seek(settings.seek)
            .map_err_context(|| translate!("dd-error-write-error"))?;
        Ok(Self { dst, settings })
    }

    /// Instantiate this struct with the named file as a destination.
    fn new_file(filename: &Path, settings: &'a Settings) -> SGResult<Self> {
        fn open_dst(path: &Path, cflags: &OConvFlags, oflags: &OFlags) -> Result<File, io::Error> {
            let mut opts = OpenOptions::new();
            opts.write(true)
                .create(!cflags.nocreat)
                .create_new(cflags.excl)
                .append(oflags.append);

            #[cfg(any(target_os = "linux"))]
            if let Some(libc_flags) = make_linux_oflags(oflags) {
                opts.custom_flags(libc_flags);
            }

            opts.open(path)
        }

        let dst = open_dst(filename, &settings.oconv, &settings.oflags).map_err_context(
            || translate!("dd-error-failed-to-open", "path" => filename.quote())
        )?;

        if !settings.oconv.notrunc {
            dst.set_len(settings.seek).ok();
        }

        Self::prepare_file(dst, settings)
    }

    fn prepare_file(dst: File, settings: &'a Settings) -> SGResult<Self> {
        let density = if settings.oconv.sparse {
            Density::Sparse
        } else {
            Density::Dense
        };
        let mut dst = Dest::File(dst, density);
        dst.seek(settings.seek)
            .map_err_context(|| translate!("dd-error-failed-to-seek"))?;
        Ok(Self { dst, settings })
    }

    /// Instantiate this struct with file descriptor as a destination.
    ///
    /// This is useful e.g. for the case when the file descriptor was
    /// already opened by the system (stdout) and has a state
    /// (current position) that shall be used.
    fn new_file_from_stdout(settings: &'a Settings) -> SGResult<Self> {
        let fx = OwnedFileDescriptorOrHandle::from(io::stdout())?;
        #[cfg(any(target_os = "linux"))]
        if let Some(libc_flags) = make_linux_oflags(&settings.oflags) {
            nix::fcntl::fcntl(
                fx.as_raw().as_fd(),
                F_SETFL(OFlag::from_bits_retain(libc_flags))
            )?;
        }

        Self::prepare_file(fx.into_file(), settings)
    }

    /// Instantiate this struct with the given named pipe as a destination.
    fn new_fifo(filename: &Path, settings: &'a Settings) -> SGResult<Self> {
        if settings.seek > 0 {
            Dest::Fifo(File::open(filename)?).seek(settings.seek)?;
        }
        if let Some(Num::Blocks(0) | Num::Bytes(0)) = settings.count {
            let dst = Dest::Sink;
            return Ok(Self { dst, settings });
        }
        let mut opts = OpenOptions::new();
        opts.write(true)
            .create(!settings.oconv.nocreat)
            .create_new(settings.oconv.excl)
            .append(settings.oflags.append);
        #[cfg(any(target_os = "linux"))]
        opts.custom_flags(make_linux_oflags(&settings.oflags).unwrap_or(0));
        let dst = Dest::Fifo(opts.open(filename)?);
        Ok(Self { dst, settings })
    }

    /// Discard the system file cache for the given portion of the output.
    ///
    /// `offset` and `len` specify a contiguous portion of the output.
    /// This function informs the kernel that the specified portion of
    /// the output file is no longer needed. If not possible, then
    /// this function prints an error message to stderr and sets the
    /// exit status code to 1.
    #[cfg_attr(not(target_os = "linux"), allow(clippy::unused_self, unused_variables))]
    fn discard_cache(&self, offset: libc::off_t, len: libc::off_t) {
        #[cfg(target_os = "linux")]
        {
            show_if_err!(
                self.dst
                    .discard_cache(offset, len)
                    .map_err_context(|| { translate!("dd-error-failed-discard-cache-output") })
            );
        }
        #[cfg(not(target_os = "linux"))]
        {
        }
    }

    /// writes a block of data. optionally retries when first try didn't complete
    ///
    /// this is needed by gnu-test: tests/dd/stats.s
    /// the write can be interrupted by a system signal.
    /// e.g. SIGUSR1 which is send to report status
    /// without retry, the data might not be fully written to destination.
    fn write_block(&mut self, chunk: &[u8]) -> io::Result<usize> {
        let full_len = chunk.len();
        let mut base_idx = 0;
        loop {
            match self.dst.write(&chunk[base_idx..]) {
                Ok(wlen) => {
                    base_idx += wlen;
                    if (base_idx >= full_len) || !self.settings.iflags.fullblock {
                        return Ok(base_idx);
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => (),
                Err(e) => return Err(e),
            }
        }
    }

    /// Write the given bytes one block at a time.
    ///
    /// This may write partial blocks (for example, if the underlying
    /// call to [`Write::write`] writes fewer than `buf.len()`
    /// bytes). The returned [`WriteStat`] object will include the
    /// number of partial and complete blocks written during execution
    /// of this function.
    fn write_blocks(&mut self, buf: &[u8]) -> io::Result<WriteStat> {
        let mut writes_complete = 0;
        let mut writes_partial = 0;
        let mut bytes_total = 0;

        for chunk in buf.chunks(self.settings.obs) {
            let wlen = self.write_block(chunk)?;
            if wlen < self.settings.obs {
                writes_partial += 1;
            } else {
                writes_complete += 1;
            }
            bytes_total += wlen;
        }

        Ok(WriteStat {
            writes_complete,
            writes_partial,
            bytes_total: bytes_total.try_into().unwrap_or(0u128),
        })
    }

    /// Flush the output to disk, if configured to do so.
    fn sync(&mut self) -> io::Result<()> {
        if self.settings.oconv.fsync {
            self.dst.fsync()
        } else if self.settings.oconv.fdatasync {
            self.dst.fdatasync()
        } else {
            Ok(())
        }
    }

    /// Truncate the underlying file to the current stream position, if possible.
    fn truncate(&mut self) -> io::Result<()> {
        self.dst.truncate()
    }
}

/// The block writer either with or without partial block buffering.
enum BlockWriter<'a> {
    /// Block writer with partial block buffering.
    ///
    /// Partial blocks are buffered until completed.
    Buffered(BufferedOutput<'a>),

    /// Block writer without partial block buffering.
    ///
    /// Partial blocks are written immediately.
    Unbuffered(Output<'a>),
}

impl BlockWriter<'_> {
    fn discard_cache(&self, offset: libc::off_t, len: libc::off_t) {
        match self {
            Self::Unbuffered(o) => o.discard_cache(offset, len),
            Self::Buffered(o) => o.discard_cache(offset, len),
        }
    }

    fn flush(&mut self) -> io::Result<WriteStat> {
        match self {
            Self::Unbuffered(_) => Ok(WriteStat::default()),
            Self::Buffered(o) => o.flush(),
        }
    }

    fn sync(&mut self) -> io::Result<()> {
        match self {
            Self::Unbuffered(o) => o.sync(),
            Self::Buffered(o) => o.sync(),
        }
    }

    /// Truncate the file to the final cursor location.
    fn truncate(&mut self) {
        match self {
            Self::Unbuffered(o) => o.truncate().ok(),
            Self::Buffered(o) => o.truncate().ok(),
        };
    }

    fn write_blocks(&mut self, buf: &[u8]) -> io::Result<WriteStat> {
        match self {
            Self::Unbuffered(o) => o.write_blocks(buf),
            Self::Buffered(o) => o.write_blocks(buf),
        }
    }
}

/// depending on the command line arguments, this function
/// informs the OS to flush/discard the caches for input and/or output file.
fn flush_caches_full_length(i: &Input, o: &Output) -> io::Result<()> {
    if i.settings.iflags.nocache {
        let offset = 0;
        #[allow(clippy::useless_conversion)]
        let len = i.src.len()?.try_into().unwrap();
        i.discard_cache(offset, len);
    }
    if i.settings.oflags.nocache {
        let offset = 0;
        #[allow(clippy::useless_conversion)]
        let len = o.dst.len()?.try_into().unwrap();
        o.discard_cache(offset, len);
    }

    Ok(())
}

/// Copy the given input data to this output, consuming both.
///
/// This method contains the main loop for the `dd` program. Bytes
/// are read in blocks from `i` and written in blocks to this
/// output. Read/write statistics are reported to stderr as
/// configured by the `status` command-line argument.
///
/// # Errors
///
/// If there is a problem reading from the input or writing to
/// this output.
fn dd_copy(mut i: Input, o: Output) -> io::Result<()> {
    let mut rstat = ReadStat::default();
    let mut wstat = WriteStat::default();

    let start = Instant::now();

    let bsize = calc_bsize(i.settings.ibs, o.settings.obs);

    let (prog_tx, rx) = mpsc::channel();
    let output_thread = thread::spawn(gen_prog_updater(rx, i.settings.status));

    let truncate = !o.settings.oconv.notrunc;

    if let Some(Num::Blocks(0) | Num::Bytes(0)) = i.settings.count {
        flush_caches_full_length(&i, &o)?;
        return finalize(
            BlockWriter::Unbuffered(o),
            rstat,
            wstat,
            start,
            &prog_tx,
            output_thread,
            truncate
        );
    }

    let mut buf = vec![BUF_INIT_BYTE; bsize];

    let alarm = Alarm::with_interval(Duration::from_secs(1));

    #[cfg(target_os = "linux")]
    let signal_handler = progress::SignalHandler::install_signal_handler(alarm.manual_trigger_fn());
    #[cfg(target_os = "linux")]
    if let Err(e) = &signal_handler {
        if Some(StatusLevel::None) != i.settings.status {
            eprintln!("{}\n\t{e}", translate!("dd-warning-signal-handler"));
        }
    }

    let mut read_offset = 0;
    let mut write_offset = 0;

    let input_nocache = i.settings.iflags.nocache;
    let output_nocache = o.settings.oflags.nocache;

    let mut o = if o.settings.buffered {
        BlockWriter::Buffered(BufferedOutput::new(o))
    } else {
        BlockWriter::Unbuffered(o)
    };

    while below_count_limit(i.settings.count, &rstat) {
        let loop_bsize = calc_loop_bsize(i.settings.count, &rstat, &wstat, i.settings.ibs, bsize);
        let rstat_update = read_helper(&mut i, &mut buf, loop_bsize)?;
        if rstat_update.is_empty() {
            break;
        }
        let wstat_update = o.write_blocks(&buf)?;

        let read_len = rstat_update.bytes_total;
        if input_nocache {
            let offset = read_offset.try_into().unwrap();
            let len = read_len.try_into().unwrap();
            i.discard_cache(offset, len);
        }
        read_offset += read_len;

        let write_len = wstat_update.bytes_total;
        if output_nocache {
            let offset = write_offset.try_into().unwrap();
            let len = write_len.try_into().unwrap();
            o.discard_cache(offset, len);
        }
        write_offset += write_len;

        rstat += rstat_update;
        wstat += wstat_update;
        match alarm.get_trigger() {
            ALARM_TRIGGER_NONE => {}
            t @ (ALARM_TRIGGER_TIMER | ALARM_TRIGGER_SIGNAL) => {
                let tp = match t {
                    ALARM_TRIGGER_TIMER => ProgUpdateType::Periodic,
                    _ => ProgUpdateType::Signal,
                };
                let prog_update = ProgUpdate::new(rstat, wstat, start.elapsed(), tp);
                prog_tx.send(prog_update).unwrap_or(());
            }
            _ => {}
        }
    }

    finalize(o, rstat, wstat, start, &prog_tx, output_thread, truncate)
}

/// Flush output, print final stats, and join with the progress thread.
fn finalize<T>(
    mut output: BlockWriter,
    rstat: ReadStat,
    wstat: WriteStat,
    start: Instant,
    prog_tx: &mpsc::Sender<ProgUpdate>,
    output_thread: thread::JoinHandle<T>,
    truncate: bool
) -> io::Result<()> {
    let wstat_update = output.flush()?;

    output.sync()?;

    if truncate {
        output.truncate();
    }

    let wstat = wstat + wstat_update;
    let prog_update = ProgUpdate::new(rstat, wstat, start.elapsed(), ProgUpdateType::Final);
    prog_tx.send(prog_update).unwrap_or(());
    output_thread
        .join()
        .expect("Failed to join with the output thread.");

    Ok(())
}

#[cfg(any(target_os = "linux"))]
#[allow(clippy::cognitive_complexity)]
fn make_linux_oflags(oflags: &OFlags) -> Option<libc::c_int> {
    let mut flag = 0;

    if oflags.append {
        flag |= libc::O_APPEND;
    }
    if oflags.direct {
        flag |= libc::O_DIRECT;
    }
    if oflags.directory {
        flag |= libc::O_DIRECTORY;
    }
    if oflags.dsync {
        flag |= libc::O_DSYNC;
    }
    if oflags.noatime {
        flag |= libc::O_NOATIME;
    }
    if oflags.noctty {
        flag |= libc::O_NOCTTY;
    }
    if oflags.nofollow {
        flag |= libc::O_NOFOLLOW;
    }
    if oflags.nonblock {
        flag |= libc::O_NONBLOCK;
    }
    if oflags.sync {
        flag |= libc::O_SYNC;
    }

    if flag == 0 { None } else { Some(flag) }
}

/// Read from an input (that is, a source of bytes) into the given buffer.
///
/// This function also performs any conversions as specified by
/// `conv=swab` or `conv=block` command-line arguments. This function
/// mutates the `buf` argument in-place. The returned [`ReadStat`]
/// indicates how many blocks were read.
fn read_helper(i: &mut Input, buf: &mut Vec<u8>, bsize: usize) -> io::Result<ReadStat> {
    fn perform_swab(buf: &mut [u8]) {
        for base in (1..buf.len()).step_by(2) {
            buf.swap(base, base - 1);
        }
    }
    buf.resize(bsize, BUF_INIT_BYTE);

    let mut rstat = match i.settings.iconv.sync {
        Some(ch) => i.fill_blocks(buf, ch)?,
        _ => i.fill_consecutive(buf)?,
    };
    if rstat.reads_complete == 0 && rstat.reads_partial == 0 {
        return Ok(rstat);
    }

    if i.settings.iconv.swab {
        perform_swab(buf);
    }

    match i.settings.iconv.mode {
        Some(ref mode) => {
            *buf = conv_block_unblock_helper(buf.clone(), mode, &mut rstat);
            Ok(rstat)
        }
        None => Ok(rstat),
    }
}

fn calc_bsize(ibs: usize, obs: usize) -> usize {
    let gcd = Gcd::gcd(ibs, obs);
    (ibs / gcd) * obs
}

/// Calculate the buffer size appropriate for this loop iteration, respecting
/// a `count=N` if present.
fn calc_loop_bsize(
    count: Option<Num>,
    rstat: &ReadStat,
    wstat: &WriteStat,
    ibs: usize,
    ideal_bsize: usize
) -> usize {
    match count {
        Some(Num::Blocks(rmax)) => {
            let rsofar = rstat.reads_complete + rstat.reads_partial;
            let rremain = rmax - rsofar;
            cmp::min(ideal_bsize as u64, rremain * ibs as u64) as usize
        }
        Some(Num::Bytes(bmax)) => {
            let bmax: u128 = bmax.into();
            let bremain: u128 = bmax - wstat.bytes_total;
            cmp::min(ideal_bsize as u128, bremain) as usize
        }
        None => ideal_bsize,
    }
}

/// Decide if the current progress is below a `count=N` limit or return
/// `true` if no such limit is set.
fn below_count_limit(count: Option<Num>, rstat: &ReadStat) -> bool {
    match count {
        Some(Num::Blocks(n)) => rstat.reads_complete + rstat.reads_partial < n,
        Some(Num::Bytes(n)) => rstat.bytes_total < n,
        None => true,
    }
}

/// Canonicalized file name of `/dev/stdout`.
///
/// For example, if this process were invoked from the command line as
/// `dd`, then this function returns the [`OsString`] form of
/// `"/dev/stdout"`. However, if this process were invoked as `dd >
/// outfile`, then this function returns the canonicalized path to
/// `outfile`, something like `"/path/to/outfile"`.
fn stdout_canonicalized() -> OsString {
    match Path::new("/dev/stdout").canonicalize() {
        Ok(p) => p.into_os_string(),
        Err(_) => OsString::from("/dev/stdout"),
    }
}

/// Decide whether stdout is being redirected to a seekable file.
///
/// For example, if this process were invoked from the command line as
///
/// ```sh
/// dd if=/dev/zero bs=1 count=10 seek=5 > /dev/sda1
/// ```
///
/// where `/dev/sda1` is a seekable block device then this function
/// would return true. If invoked as
///
/// ```sh
/// dd if=/dev/zero bs=1 count=10 seek=5
/// ```
///
/// then this function would return false.
fn is_stdout_redirected_to_seekable_file() -> bool {
    let s = stdout_canonicalized();
    let p = Path::new(&s);
    match File::open(p) {
        Ok(mut f) => {
            f.stream_position().is_ok() && f.seek(SeekFrom::End(0)).is_ok() && f.rewind().is_ok()
        }
        Err(_) => false,
    }
}

/// Try to get the len if it is a block device
fn try_get_len_of_block_device(file: &mut File) -> io::Result<Option<u64>> {
    let ftype = file.metadata()?.file_type();
    if !ftype.is_block_device() {
        return Ok(None);
    }

    let len = file.seek(SeekFrom::End(0))?;
    file.rewind()?;
    Ok(Some(len))
}

/// Decide whether the named file is a named pipe, also known as a FIFO.
fn is_fifo(filename: &str) -> bool {
    if let Ok(metadata) = std::fs::metadata(filename) {
        if metadata.file_type().is_fifo() {
            return true;
        }
    }
    false
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio"])?;

    let settings: Settings = Parser::new().parse(
        matches
            .get_many::<String>(options::OPERANDS)
            .unwrap_or_default()
    )?;

    let i = match settings.infile {
        Some(ref infile) if is_fifo(infile) => Input::new_fifo(Path::new(&infile), &settings)?,
        Some(ref infile) => Input::new_file(Path::new(&infile), &settings)?,
        None => Input::new_stdin(&settings)?,
    };
    let o = match settings.outfile {
        Some(ref outfile) if is_fifo(outfile) => Output::new_fifo(Path::new(&outfile), &settings)?,
        Some(ref outfile) => Output::new_file(Path::new(&outfile), &settings)?,
        None if is_stdout_redirected_to_seekable_file() => Output::new_file_from_stdout(&settings)?,
        None => Output::new_stdout(&settings)?,
    };
    dd_copy(i, o).map_err_context(|| translate!("dd-error-io-error"))
}

pub fn sg_app() -> Command {
    Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("dd-about"))
        .override_usage(format_usage(&translate!("dd-usage")))
        .after_help(translate!("dd-after-help"))
        .infer_long_args(true)
        .arg(Arg::new(options::OPERANDS).num_args(1..))
}

#[cfg(test)]
mod tests {
    use crate::{Output, Parser, calc_bsize};

    use std::path::Path;

    #[test]
    fn bsize_test_primes() {
        let (n, m) = (7901, 7919);
        let res = calc_bsize(n, m);
        assert_eq!(res % n, 0);
        assert_eq!(res % m, 0);

        assert_eq!(res, n * m);
    }

    #[test]
    fn bsize_test_rel_prime_obs_greater() {
        let (n, m) = (7 * 5119, 13 * 5119);
        let res = calc_bsize(n, m);
        assert_eq!(res % n, 0);
        assert_eq!(res % m, 0);

        assert_eq!(res, 7 * 13 * 5119);
    }

    #[test]
    fn bsize_test_rel_prime_ibs_greater() {
        let (n, m) = (13 * 5119, 7 * 5119);
        let res = calc_bsize(n, m);
        assert_eq!(res % n, 0);
        assert_eq!(res % m, 0);

        assert_eq!(res, 7 * 13 * 5119);
    }

    #[test]
    fn bsize_test_3fac_rel_prime() {
        let (n, m) = (11 * 13 * 5119, 7 * 11 * 5119);
        let res = calc_bsize(n, m);
        assert_eq!(res % n, 0);
        assert_eq!(res % m, 0);

        assert_eq!(res, 7 * 11 * 13 * 5119);
    }

    #[test]
    fn bsize_test_ibs_greater() {
        let (n, m) = (512 * 1024, 256 * 1024);
        let res = calc_bsize(n, m);
        assert_eq!(res % n, 0);
        assert_eq!(res % m, 0);

        assert_eq!(res, n);
    }

    #[test]
    fn bsize_test_obs_greater() {
        let (n, m) = (256 * 1024, 512 * 1024);
        let res = calc_bsize(n, m);
        assert_eq!(res % n, 0);
        assert_eq!(res % m, 0);

        assert_eq!(res, m);
    }

    #[test]
    fn bsize_test_bs_eq() {
        let (n, m) = (1024, 1024);
        let res = calc_bsize(n, m);
        assert_eq!(res % n, 0);
        assert_eq!(res % m, 0);

        assert_eq!(res, m);
    }

    #[test]
    fn test_nocreat_causes_failure_when_ofile_doesnt_exist() {
        let args = &["conv=nocreat", "of=not-a-real.file"];
        let settings = Parser::new().parse(args).unwrap();
        assert!(
            Output::new_file(Path::new(settings.outfile.as_ref().unwrap()), &settings).is_err()
        );
    }
}

