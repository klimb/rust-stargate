









pub mod args;
pub mod chunks;
mod follow;
mod parse;
mod paths;
mod platform;
pub mod text;

pub use args::sg_app;
use args::{FilterMode, Settings, Signum, parse_args};
use chunks::ReverseChunks;
use follow::Observer;
use memchr::{memchr_iter, memrchr_iter};
use paths::{FileExtTail, HeaderPrinter, Input, InputKind};
use same_file::Handle;
use std::cmp::Ordering;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, ErrorKind, Read, Seek, SeekFrom, Write, stdin, stdout};
use std::path::{Path, PathBuf};
use sgcore::display::Quotable;
use sgcore::error::{FromIo, SGResult, SGSimpleError, get_exit_code, set_exit_code};
use sgcore::translate;

use sgcore::{show, show_error};

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    sgcore::pledge::apply_pledge(&["stdio", "rpath"])?;

    let settings = parse_args(args)?;

    settings.check_warnings();

    match settings.verify() {
        args::VerificationResult::CannotFollowStdinByName => {
            return Err(SGSimpleError::new(
                1,
                translate!("tail-error-cannot-follow-stdin-by-name", "stdin" => text::DASH.quote())
            ));
        }
        args::VerificationResult::NoOutput => return Ok(()),
        args::VerificationResult::Ok => {}
    }

    sg_tail(&settings)
}

fn sg_tail(settings: &Settings) -> SGResult<()> {
    let mut printer = HeaderPrinter::new(settings.verbose, true);
    let mut observer = Observer::from(settings);

    observer.start(settings)?;
    for input in &settings.inputs.clone() {
        match input.kind() {
            InputKind::Stdin => {
                tail_stdin(settings, &mut printer, input, &mut observer)?;
            }
            InputKind::File(path) if cfg!(unix) && path == &PathBuf::from(text::DEV_STDIN) => {
                tail_stdin(settings, &mut printer, input, &mut observer)?;
            }
            InputKind::File(path) => {
                tail_file(settings, &mut printer, input, path, &mut observer, 0)?;
            }
        }
    }

    if settings.follow.is_some() {
        if !settings.has_only_stdin() || settings.pid != 0 {
            follow::follow(observer, settings)?;
        }
    }

    if get_exit_code() > 0 && paths::stdin_is_bad_fd() {
        show_error!("{}: {}", text::DASH, translate!("tail-bad-fd"));
    }

    Ok(())
}

fn tail_file(
    settings: &Settings,
    header_printer: &mut HeaderPrinter,
    input: &Input,
    path: &Path,
    observer: &mut Observer,
    offset: u64
) -> SGResult<()> {
    if !path.exists() {
        set_exit_code(1);
        show_error!(
            "{}",
            translate!("tail-error-cannot-open-no-such-file", "file" => input.display_name.clone(), "error" => translate!("tail-no-such-file-or-directory"))
        );
        observer.add_bad_path(path, input.display_name.as_str(), false)?;
    } else if path.is_dir() {
        set_exit_code(1);

        header_printer.print_input(input);
        let err_msg = translate!("tail-is-a-directory");

        show_error!(
            "{}",
            translate!("tail-error-reading-file", "file" => input.display_name.clone(), "error" => err_msg)
        );
        if settings.follow.is_some() {
            let msg = if settings.retry {
                ""
            } else {
                &translate!("tail-giving-up-on-this-name")
            };
            show_error!(
                "{}",
                translate!("tail-error-cannot-follow-file-type", "file" => input.display_name.clone(), "msg" => msg)
            );
        }
        if !observer.follow_name_retry() {
            return Ok(());
        }
        observer.add_bad_path(path, input.display_name.as_str(), false)?;
    } else {
        match File::open(path) {
            Ok(mut file) => {
                let st = file.metadata()?;
                let blksize_limit = sgcore::fs::sane_blksize::sane_blksize_from_metadata(&st);
                header_printer.print_input(input);
                let mut reader;
                if !settings.presume_input_pipe
                    && file.is_seekable(if input.is_stdin() { offset } else { 0 })
                    && (!st.is_file() || st.len() > blksize_limit)
                {
                    bounded_tail(&mut file, settings);
                    reader = BufReader::new(file);
                } else {
                    reader = BufReader::new(file);
                    unbounded_tail(&mut reader, settings)?;
                }
                if input.is_tailable() {
                    observer.add_path(
                        path,
                        input.display_name.as_str(),
                        Some(Box::new(reader)),
                        true
                    )?;
                } else {
                    observer.add_bad_path(path, input.display_name.as_str(), false)?;
                }
            }
            Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                observer.add_bad_path(path, input.display_name.as_str(), false)?;
                show!(e.map_err_context(|| {
                    translate!("tail-error-cannot-open-for-reading", "file" => input.display_name.clone())
                }));
            }
            Err(e) => {
                observer.add_bad_path(path, input.display_name.as_str(), false)?;
                return Err(e.map_err_context(|| {
                    translate!("tail-error-cannot-open-for-reading", "file" => input.display_name.clone())
                }));
            }
        }
    }

    Ok(())
}

fn tail_stdin(
    settings: &Settings,
    header_printer: &mut HeaderPrinter,
    input: &Input,
    observer: &mut Observer
) -> SGResult<()> {
    #[cfg(target_os = "macos")]
    {
        if let Ok(mut stdin_handle) = Handle::stdin() {
            if let Ok(meta) = stdin_handle.as_file_mut().metadata() {
                if meta.file_type().is_dir() {
                    set_exit_code(1);
                    show_error!(
                        "{}",
                        translate!("tail-error-cannot-open-no-such-file", "file" => input.display_name.clone(), "error" => translate!("tail-no-such-file-or-directory"))
                    );
                    return Ok(());
                }
            }
        }
    }

    match input.resolve() {
        Some(path) => {
            let mut stdin_offset = 0;
            if cfg!(unix) {
                if let Ok(mut stdin_handle) = Handle::stdin() {
                    if let Ok(offset) = stdin_handle.as_file_mut().stream_position() {
                        stdin_offset = offset;
                    }
                }
            }
            tail_file(
                settings,
                header_printer,
                input,
                &path,
                observer,
                stdin_offset
            )?;
        }
        None => {
            header_printer.print_input(input);
            if paths::stdin_is_bad_fd() {
                set_exit_code(1);
                show_error!(
                    "{}",
                    translate!("tail-error-cannot-fstat", "file" => translate!("tail-stdin-header"), "error" => translate!("tail-bad-fd"))
                );
                if settings.follow.is_some() {
                    show_error!(
                        "{}",
                        translate!("tail-error-reading-file", "file" => translate!("tail-stdin-header"), "error" => translate!("tail-bad-fd"))
                    );
                }
            } else {
                let mut reader = BufReader::new(stdin());
                unbounded_tail(&mut reader, settings)?;
                observer.add_stdin(input.display_name.as_str(), Some(Box::new(reader)), true)?;
            }
        }
    }

    Ok(())
}

/// Find the index after the given number of instances of a given byte.
///
/// This function reads through a given reader until `num_delimiters`
/// instances of `delimiter` have been seen, returning the index of
/// the byte immediately following that delimiter. If there are fewer
/// than `num_delimiters` instances of `delimiter`, this returns the
/// total number of bytes read from the `reader` until EOF.
///
/// # Errors
///
/// This function returns an error if there is an error during reading
/// from `reader`.
///
/// # Examples
///
/// Basic usage:
///
/// ```rust,ignore
/// use std::io::Cursor;
///
/// let mut reader = Cursor::new("a\nb\nc\nd\ne\n");
/// let i = forwards_thru_file(&mut reader, 2, b'\n').unwrap();
/// assert_eq!(i, 4);
/// ```
///
/// If `num_delimiters` is zero, then this function always returns
/// zero:
///
/// ```rust,ignore
/// use std::io::Cursor;
///
/// let mut reader = Cursor::new("a\n");
/// let i = forwards_thru_file(&mut reader, 0, b'\n').unwrap();
/// assert_eq!(i, 0);
/// ```
///
/// If there are fewer than `num_delimiters` instances of `delimiter`
/// in the reader, then this function returns the total number of
/// bytes read:
///
/// ```rust,ignore
/// use std::io::Cursor;
///
/// let mut reader = Cursor::new("a\n");
/// let i = forwards_thru_file(&mut reader, 2, b'\n').unwrap();
/// assert_eq!(i, 2);
/// ```
fn forwards_thru_file(
    reader: &mut impl Read,
    num_delimiters: u64,
    delimiter: u8
) -> io::Result<usize> {
    if num_delimiters == 0 {
        return Ok(0);
    }
    let mut buf = [0; 32 * 1024];
    let mut total = 0;
    let mut count = 0;
    loop {
        match reader.read(&mut buf) {
            Ok(0) => return Ok(total),
            Ok(n) => {
                for offset in memchr_iter(delimiter, &buf[..n]) {
                    count += 1;
                    if count == num_delimiters {
                        return Ok(total + offset + 1);
                    }
                }
                total += n;
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => (),
            Err(e) => return Err(e),
        }
    }
}

/// Iterate over bytes in the file, in reverse, until we find the
/// `num_delimiters` instance of `delimiter`. The `file` is left seek'd to the
/// position just after that delimiter.
fn backwards_thru_file(file: &mut File, num_delimiters: u64, delimiter: u8) {
    let mut counter = 0;
    let mut first_slice = true;
    for slice in ReverseChunks::new(file) {
        let mut iter = memrchr_iter(delimiter, &slice);

        if first_slice {
            if let Some(c) = slice.last() {
                if *c == delimiter {
                    iter.next();
                }
            }
            first_slice = false;
        }

        for i in iter {
            counter += 1;
            if counter >= num_delimiters {
                assert_eq!(counter, num_delimiters);
                file.seek(SeekFrom::Current((i + 1) as i64)).unwrap();
                return;
            }
        }
    }
}

/// When tail'ing a file, we do not need to read the whole file from start to
/// finish just to find the last n lines or bytes. Instead, we can seek to the
/// end of the file, and then read the file "backwards" in blocks of size
/// `BLOCK_SIZE` until we find the location of the first line/byte. This ends up
/// being a nice performance win for very large files.
fn bounded_tail(file: &mut File, settings: &Settings) {
    debug_assert!(!settings.presume_input_pipe);
    let mut limit = None;

    match &settings.mode {
        FilterMode::Lines(Signum::Negative(count), delimiter) => {
            backwards_thru_file(file, *count, *delimiter);
        }
        FilterMode::Lines(Signum::Positive(count), delimiter) if count > &1 => {
            let i = forwards_thru_file(file, *count - 1, *delimiter).unwrap();
            file.seek(SeekFrom::Start(i as u64)).unwrap();
        }
        FilterMode::Lines(Signum::MinusZero, _) => {
            return;
        }
        FilterMode::Bytes(Signum::Negative(count)) => {
            file.seek(SeekFrom::End(-(*count as i64))).unwrap();
            limit = Some(*count);
        }
        FilterMode::Bytes(Signum::Positive(count)) if count > &1 => {
            file.seek(SeekFrom::Start(*count - 1)).unwrap();
        }
        FilterMode::Bytes(Signum::MinusZero) => {
            return;
        }
        _ => {}
    }

    print_target_section(file, limit);
}

fn unbounded_tail<T: Read>(reader: &mut BufReader<T>, settings: &Settings) -> SGResult<()> {
    let mut writer = BufWriter::new(stdout().lock());
    match &settings.mode {
        FilterMode::Lines(Signum::Negative(count), sep) => {
            let mut chunks = chunks::LinesChunkBuffer::new(*sep, *count);
            chunks.fill(reader)?;
            chunks.print(&mut writer)?;
        }
        FilterMode::Lines(Signum::PlusZero | Signum::Positive(1), _) => {
            io::copy(reader, &mut writer)?;
        }
        FilterMode::Lines(Signum::Positive(count), sep) => {
            let mut num_skip = *count - 1;
            let mut chunk = chunks::LinesChunk::new(*sep);
            while chunk.fill(reader)?.is_some() {
                let lines = chunk.get_lines() as u64;
                if lines < num_skip {
                    num_skip -= lines;
                } else {
                    break;
                }
            }
            if chunk.has_data() {
                chunk.print_lines(&mut writer, num_skip as usize)?;
                io::copy(reader, &mut writer)?;
            }
        }
        FilterMode::Bytes(Signum::Negative(count)) => {
            let mut chunks = chunks::BytesChunkBuffer::new(*count);
            chunks.fill(reader)?;
            chunks.print(&mut writer)?;
        }
        FilterMode::Bytes(Signum::PlusZero | Signum::Positive(1)) => {
            io::copy(reader, &mut writer)?;
        }
        FilterMode::Bytes(Signum::Positive(count)) => {
            let mut num_skip = *count - 1;
            let mut chunk = chunks::BytesChunk::new();
            loop {
                if let Some(bytes) = chunk.fill(reader)? {
                    let bytes: u64 = bytes as u64;
                    match bytes.cmp(&num_skip) {
                        Ordering::Less => num_skip -= bytes,
                        Ordering::Equal => {
                            break;
                        }
                        Ordering::Greater => {
                            writer.write_all(chunk.get_buffer_with(num_skip as usize))?;
                            break;
                        }
                    }
                } else {
                    return Ok(());
                }
            }

            io::copy(reader, &mut writer)?;
        }
        _ => {}
    }
    writer.flush()?;

    Ok(())
}

fn print_target_section<R>(file: &mut R, limit: Option<u64>)
where
    R: Read + ?Sized,
{
    let stdout = stdout();
    let mut stdout = stdout.lock();
    if let Some(limit) = limit {
        let mut reader = file.take(limit);
        io::copy(&mut reader, &mut stdout).unwrap();
    } else {
        io::copy(file, &mut stdout).unwrap();
    }
}

#[cfg(test)]
mod tests {

    use crate::forwards_thru_file;
    use std::io::Cursor;

    #[test]
    fn test_forwards_thru_file_zero() {
        let mut reader = Cursor::new("a\n");
        let i = forwards_thru_file(&mut reader, 0, b'\n').unwrap();
        assert_eq!(i, 0);
    }

    #[test]
    fn test_forwards_thru_file_basic() {
        let mut reader = Cursor::new("a\nb\nc\nd\ne\n");
        let i = forwards_thru_file(&mut reader, 2, b'\n').unwrap();
        assert_eq!(i, 4);
    }

    #[test]
    fn test_forwards_thru_file_past_end() {
        let mut reader = Cursor::new("x\n");
        let i = forwards_thru_file(&mut reader, 2, b'\n').unwrap();
        assert_eq!(i, 2);
    }
}

