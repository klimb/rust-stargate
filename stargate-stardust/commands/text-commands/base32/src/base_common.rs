

use clap::{Arg, ArgAction, Command};
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, ErrorKind, Read, Seek, Write};
use std::path::{Path, PathBuf};
use sgcore::display::Quotable;
use sgcore::encoding::{
    BASE2LSBF, BASE2MSBF, Base32Wrapper, Base58Wrapper, Base64SimdWrapper, EncodingWrapper, Format,
    SupportsFastDecodeAndEncode, Z85Wrapper,
    for_base_common::{BASE32, BASE32HEX, BASE64URL, HEXUPPER_PERMISSIVE},
};
use sgcore::error::{FromIo, SGResult, SGSimpleError, SGUsageError};
use sgcore::format_usage;
use sgcore::translate;

pub const BASE_CMD_PARSE_ERROR: i32 = 1;

/// Encoded output will be formatted in lines of this length (the last line can be shorter)
///
/// Other implementations default to 76
///
/// This default is only used if no "-w"/"--wrap" argument is passed
pub const WRAP_DEFAULT: usize = 76;

pub struct Config {
    pub decode: bool,
    pub ignore_garbage: bool,
    pub wrap_cols: Option<usize>,
    pub to_read: Option<PathBuf>,
}

pub mod options {
    pub static DECODE: &str = "decode";
    pub static WRAP: &str = "wrap";
    pub static IGNORE_GARBAGE: &str = "ignore-garbage";
    pub static FILE: &str = "file";
}

impl Config {
    pub fn from(options: &clap::ArgMatches) -> SGResult<Self> {
        let to_read = match options.get_many::<OsString>(options::FILE) {
            Some(mut values) => {
                let name = values.next().unwrap();

                if let Some(extra_op) = values.next() {
                    return Err(SGUsageError::new(
                        BASE_CMD_PARSE_ERROR,
                        translate!("base-common-extra-operand", "operand" => extra_op.to_string_lossy().quote())
                    ));
                }

                if name == "-" {
                    None
                } else {
                    let path = Path::new(name);

                    if !path.exists() {
                        return Err(SGSimpleError::new(
                            BASE_CMD_PARSE_ERROR,
                            translate!("base-common-no-such-file", "file" => path.maybe_quote())
                        ));
                    }

                    Some(path.to_owned())
                }
            }
            None => None,
        };

        let wrap_cols = options
            .get_one::<String>(options::WRAP)
            .map(|num| {
                num.parse::<usize>().map_err(|_| {
                    SGSimpleError::new(
                        BASE_CMD_PARSE_ERROR,
                        translate!("base-common-invalid-wrap-size", "size" => num.quote())
                    )
                })
            })
            .transpose()?;

        Ok(Self {
            decode: options.get_flag(options::DECODE),
            ignore_garbage: options.get_flag(options::IGNORE_GARBAGE),
            wrap_cols,
            to_read,
        })
    }
}

pub fn parse_base_cmd_args(
    args: impl sgcore::Args,
    about: &'static str,
    usage: &str
) -> SGResult<Config> {
    let command = base_app(about, usage);
    let matches = sgcore::clap_localization::handle_clap_result(command, args)?;
    Config::from(&matches)
}

pub fn base_app(about: &'static str, usage: &str) -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .about(about)
        .override_usage(format_usage(usage))
        .infer_long_args(true);
    sgcore::clap_localization::configure_localized_command(cmd)
        .arg(
            Arg::new(options::DECODE)
                .short('d')
                .visible_short_alias('D')
                .long(options::DECODE)
                .help(translate!("base-common-help-decode"))
                .action(ArgAction::SetTrue)
                .overrides_with(options::DECODE)
        )
        .arg(
            Arg::new(options::IGNORE_GARBAGE)
                .short('i')
                .long(options::IGNORE_GARBAGE)
                .help(translate!("base-common-help-ignore-garbage"))
                .action(ArgAction::SetTrue)
                .overrides_with(options::IGNORE_GARBAGE)
        )
        .arg(
            Arg::new(options::WRAP)
                .short('w')
                .long(options::WRAP)
                .value_name("COLS")
                .help(translate!("base-common-help-wrap", "default" => WRAP_DEFAULT))
                .overrides_with(options::WRAP)
        )
        .arg(
            Arg::new(options::FILE)
                .index(1)
                .action(ArgAction::Append)
                .value_parser(clap::value_parser!(OsString))
                .value_hint(clap::ValueHint::FilePath)
        )
}

/// A trait alias for types that implement both `Read` and `Seek`.
pub trait ReadSeek: Read + Seek {}

/// Automatically implement the `ReadSeek` trait for any type that implements both `Read` and `Seek`.
impl<T: Read + Seek> ReadSeek for T {}

pub fn get_input(config: &Config) -> SGResult<Box<dyn ReadSeek>> {
    match &config.to_read {
        Some(path_buf) => {
            let file =
                File::open(path_buf).map_err_context(|| path_buf.maybe_quote().to_string())?;
            Ok(Box::new(file))
        }
        None => {
            let mut buffer = Vec::new();
            io::stdin().read_to_end(&mut buffer)?;
            Ok(Box::new(io::Cursor::new(buffer)))
        }
    }
}

/// Determines if the input buffer contains any padding ('=') ignoring trailing whitespace.
fn read_and_has_padding<R: Read>(input: &mut R) -> SGResult<(bool, Vec<u8>)> {
    let mut buf = Vec::new();
    input
        .read_to_end(&mut buf)
        .map_err(|err| SGSimpleError::new(1, format_read_error(err.kind())))?;

    let has_padding = buf.contains(&b'=');

    Ok((has_padding, buf))
}

pub fn handle_input<R: Read + Seek>(input: &mut R, format: Format, config: Config) -> SGResult<()> {
    let (has_padding, read) = read_and_has_padding(input)?;

    let supports_fast_decode_and_encode =
        get_supports_fast_decode_and_encode(format, config.decode, has_padding);

    let supports_fast_decode_and_encode_ref = supports_fast_decode_and_encode.as_ref();
    let mut stdout_lock = io::stdout().lock();
    let result = if config.decode {
        fast_decode::fast_decode(
            read,
            &mut stdout_lock,
            supports_fast_decode_and_encode_ref,
            config.ignore_garbage
        )
    } else {
        fast_encode::fast_encode(
            read,
            &mut stdout_lock,
            supports_fast_decode_and_encode_ref,
            config.wrap_cols
        )
    };

    match (result, stdout_lock.flush()) {
        (res, Ok(())) => res,
        (Ok(_), Err(err)) => Err(err.into()),
        (Err(original), Err(_)) => Err(original),
    }
}

pub fn get_supports_fast_decode_and_encode(
    format: Format,
    decode: bool,
    has_padding: bool
) -> Box<dyn SupportsFastDecodeAndEncode> {
    const BASE16_VALID_DECODING_MULTIPLE: usize = 2;
    const BASE2_VALID_DECODING_MULTIPLE: usize = 8;
    const BASE32_VALID_DECODING_MULTIPLE: usize = 8;
    const BASE64_VALID_DECODING_MULTIPLE: usize = 4;

    const BASE16_UNPADDED_MULTIPLE: usize = 1;
    const BASE2_UNPADDED_MULTIPLE: usize = 1;
    const BASE32_UNPADDED_MULTIPLE: usize = 5;
    const BASE64_UNPADDED_MULTIPLE: usize = 3;

    match format {
        Format::Base16 => Box::from(EncodingWrapper::new(
            HEXUPPER_PERMISSIVE,
            BASE16_VALID_DECODING_MULTIPLE,
            BASE16_UNPADDED_MULTIPLE,
            b"0123456789ABCDEFabcdef"
        )),
        Format::Base2Lsbf => Box::from(EncodingWrapper::new(
            BASE2LSBF,
            BASE2_VALID_DECODING_MULTIPLE,
            BASE2_UNPADDED_MULTIPLE,
            b"01"
        )),
        Format::Base2Msbf => Box::from(EncodingWrapper::new(
            BASE2MSBF,
            BASE2_VALID_DECODING_MULTIPLE,
            BASE2_UNPADDED_MULTIPLE,
            b"01"
        )),
        Format::Base32 => Box::from(Base32Wrapper::new(
            BASE32,
            BASE32_VALID_DECODING_MULTIPLE,
            BASE32_UNPADDED_MULTIPLE,
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567="
        )),
        Format::Base32Hex => Box::from(Base32Wrapper::new(
            BASE32HEX,
            BASE32_VALID_DECODING_MULTIPLE,
            BASE32_UNPADDED_MULTIPLE,
            b"0123456789ABCDEFGHIJKLMNOPQRSTUV="
        )),
        Format::Base64 => {
            let alphabet: &[u8] = if has_padding {
                &b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789+/="[..]
            } else {
                &b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789+/"[..]
            };
            let use_padding = !decode || has_padding;
            Box::from(Base64SimdWrapper::new(
                use_padding,
                BASE64_VALID_DECODING_MULTIPLE,
                BASE64_UNPADDED_MULTIPLE,
                alphabet
            ))
        }
        Format::Base64Url => Box::from(EncodingWrapper::new(
            BASE64URL,
            BASE64_VALID_DECODING_MULTIPLE,
            BASE64_UNPADDED_MULTIPLE,
            b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789=_-"
        )),
        Format::Z85 => Box::from(Z85Wrapper {}),
        Format::Base58 => Box::from(Base58Wrapper {}),
    }
}

pub mod fast_encode {
    use crate::base_common::WRAP_DEFAULT;
    use std::{
        cmp::min,
        collections::VecDeque,
        io::{self, Write},
        num::NonZeroUsize,
    };
    use sgcore::{encoding::SupportsFastDecodeAndEncode, error::SGResult};

    struct LineWrapping {
        line_length: NonZeroUsize,
        print_buffer: Vec<u8>,
    }

    fn encode_in_chunks_to_buffer(
        supports_fast_decode_and_encode: &dyn SupportsFastDecodeAndEncode,
        read_buffer: &[u8],
        encoded_buffer: &mut VecDeque<u8>
    ) -> SGResult<()> {
        supports_fast_decode_and_encode.encode_to_vec_deque(read_buffer, encoded_buffer)?;
        Ok(())
    }

    fn write_without_line_breaks(
        encoded_buffer: &mut VecDeque<u8>,
        output: &mut dyn Write,
        is_cleanup: bool,
        empty_wrap: bool
    ) -> io::Result<()> {
        output.write_all(encoded_buffer.make_contiguous())?;

        if is_cleanup {
            if !empty_wrap {
                output.write_all(b"\n")?;
            }
        } else {
            encoded_buffer.clear();
        }

        Ok(())
    }

    fn write_with_line_breaks(
        &mut LineWrapping {
            ref line_length,
            ref mut print_buffer,
        }: &mut LineWrapping,
        encoded_buffer: &mut VecDeque<u8>,
        output: &mut dyn Write,
        is_cleanup: bool
    ) -> io::Result<()> {
        let line_length = line_length.get();

        let make_contiguous_result = encoded_buffer.make_contiguous();

        let chunks_exact = make_contiguous_result.chunks_exact(line_length);

        let mut bytes_added_to_print_buffer = 0;

        for sl in chunks_exact {
            bytes_added_to_print_buffer += sl.len();

            print_buffer.extend_from_slice(sl);
            print_buffer.push(b'\n');
        }

        output.write_all(print_buffer)?;

        drop(encoded_buffer.drain(..bytes_added_to_print_buffer));

        if is_cleanup {
            if encoded_buffer.is_empty() {
            } else {
                output.write_all(encoded_buffer.make_contiguous())?;
                output.write_all(b"\n")?;
            }
        } else {
            print_buffer.clear();
        }

        Ok(())
    }

    fn write_to_output(
        line_wrapping: &mut Option<LineWrapping>,
        encoded_buffer: &mut VecDeque<u8>,
        output: &mut dyn Write,
        is_cleanup: bool,
        empty_wrap: bool
    ) -> io::Result<()> {
        if let &mut Some(ref mut li) = line_wrapping {
            write_with_line_breaks(li, encoded_buffer, output, is_cleanup)?;
        } else {
            write_without_line_breaks(encoded_buffer, output, is_cleanup, empty_wrap)?;
        }

        Ok(())
    }

    pub fn fast_encode(
        input: Vec<u8>,
        output: &mut dyn Write,
        supports_fast_decode_and_encode: &dyn SupportsFastDecodeAndEncode,
        wrap: Option<usize>
    ) -> SGResult<()> {

        const ENCODE_IN_CHUNKS_OF_SIZE_MULTIPLE: usize = 1_024;

        let encode_in_chunks_of_size =
            supports_fast_decode_and_encode.unpadded_multiple() * ENCODE_IN_CHUNKS_OF_SIZE_MULTIPLE;

        assert!(encode_in_chunks_of_size > 0);

        let mut line_wrapping = match wrap {
            Some(0) => None,
            Some(an) => Some(LineWrapping {
                line_length: NonZeroUsize::new(an).unwrap(),
                print_buffer: Vec::<u8>::new(),
            }),
            None => Some(LineWrapping {
                line_length: NonZeroUsize::new(WRAP_DEFAULT).unwrap(),
                print_buffer: Vec::<u8>::new(),
            }),
        };

        let input_size = input.len();

        let mut leftover_buffer = VecDeque::<u8>::new();

        let mut encoded_buffer = VecDeque::<u8>::new();

        input
            .iter()
            .enumerate()
            .step_by(encode_in_chunks_of_size)
            .map(|(idx, _)| {
                &input[idx..min(input_size, idx + encode_in_chunks_of_size)]
            })
            .map(|buffer| {
                if buffer.len() < encode_in_chunks_of_size {
                    leftover_buffer.extend(buffer);
                    assert!(leftover_buffer.len() < encode_in_chunks_of_size);
                    return None;
                }
                Some(buffer)
            })
            .for_each(|buffer| {
                if let Some(read_buffer) = buffer {
                    assert_eq!(read_buffer.len(), encode_in_chunks_of_size);
                    encode_in_chunks_to_buffer(
                        supports_fast_decode_and_encode,
                        read_buffer,
                        &mut encoded_buffer
                    )
                    .unwrap();
                    write_to_output(
                        &mut line_wrapping,
                        &mut encoded_buffer,
                        output,
                        false,
                        wrap == Some(0)
                    )
                    .unwrap();
                }
            });

        {
            supports_fast_decode_and_encode
                .encode_to_vec_deque(leftover_buffer.make_contiguous(), &mut encoded_buffer)?;

            write_to_output(
                &mut line_wrapping,
                &mut encoded_buffer,
                output,
                true,
                wrap == Some(0)
            )?;
        }
        Ok(())
    }
}

pub mod fast_decode {
    use std::io::{self, Write};
    use sgcore::{
        encoding::SupportsFastDecodeAndEncode,
        error::{SGResult, SGSimpleError},
    };

    fn alphabet_lookup(alphabet: &[u8]) -> [bool; 256] {
        let mut table = [false; 256];

        for &byte in alphabet {
            table[usize::from(byte)] = true;
        }

        table
    }

    fn decode_in_chunks_to_buffer(
        supports_fast_decode_and_encode: &dyn SupportsFastDecodeAndEncode,
        read_buffer_filtered: &[u8],
        decoded_buffer: &mut Vec<u8>
    ) -> SGResult<()> {
        supports_fast_decode_and_encode.decode_into_vec(read_buffer_filtered, decoded_buffer)?;
        Ok(())
    }

    fn write_to_output(decoded_buffer: &mut Vec<u8>, output: &mut dyn Write) -> io::Result<()> {
        output.write_all(decoded_buffer.as_slice())?;
        output.flush()?;

        decoded_buffer.clear();

        Ok(())
    }

    fn flush_ready_chunks(
        buffer: &mut Vec<u8>,
        block_limit: usize,
        valid_multiple: usize,
        supports_fast_decode_and_encode: &dyn SupportsFastDecodeAndEncode,
        decoded_buffer: &mut Vec<u8>,
        output: &mut dyn Write
    ) -> SGResult<()> {
        while buffer.len() >= valid_multiple {
            let take = buffer.len().min(block_limit);
            let aligned_take = take - (take % valid_multiple);

            if aligned_take < valid_multiple {
                break;
            }

            decode_in_chunks_to_buffer(
                supports_fast_decode_and_encode,
                &buffer[..aligned_take],
                decoded_buffer
            )?;

            write_to_output(decoded_buffer, output)?;

            buffer.drain(..aligned_take);
        }

        Ok(())
    }

    pub fn fast_decode(
        input: Vec<u8>,
        output: &mut dyn Write,
        supports_fast_decode_and_encode: &dyn SupportsFastDecodeAndEncode,
        ignore_garbage: bool
    ) -> SGResult<()> {
        const DECODE_IN_CHUNKS_OF_SIZE_MULTIPLE: usize = 1_024;

        let alphabet = supports_fast_decode_and_encode.alphabet();
        let alphabet_table = alphabet_lookup(alphabet);
        let valid_multiple = supports_fast_decode_and_encode.valid_decoding_multiple();
        let decode_in_chunks_of_size = valid_multiple * DECODE_IN_CHUNKS_OF_SIZE_MULTIPLE;

        assert!(decode_in_chunks_of_size > 0);
        assert!(valid_multiple > 0);

        let mut decoded_buffer = Vec::<u8>::new();

        let mut buffer = Vec::with_capacity(decode_in_chunks_of_size);

        let supports_partial_decode = supports_fast_decode_and_encode.supports_partial_decode();

        for &byte in &input {
            if byte == b'\n' || byte == b'\r' {
                continue;
            }

            if alphabet_table[usize::from(byte)] {
                buffer.push(byte);
            } else if ignore_garbage {
                continue;
            } else {
                return Err(SGSimpleError::new(1, "error: invalid input".to_owned()));
            }

            if supports_partial_decode {
                flush_ready_chunks(
                    &mut buffer,
                    decode_in_chunks_of_size,
                    valid_multiple,
                    supports_fast_decode_and_encode,
                    &mut decoded_buffer,
                    output
                )?;
            } else if buffer.len() == decode_in_chunks_of_size {
                decode_in_chunks_to_buffer(
                    supports_fast_decode_and_encode,
                    &buffer,
                    &mut decoded_buffer
                )?;
                write_to_output(&mut decoded_buffer, output)?;
                buffer.clear();
            }
        }

        if supports_partial_decode {
            flush_ready_chunks(
                &mut buffer,
                decode_in_chunks_of_size,
                valid_multiple,
                supports_fast_decode_and_encode,
                &mut decoded_buffer,
                output
            )?;
        }

        if !buffer.is_empty() {
            let mut owned_chunk: Option<Vec<u8>> = None;
            let mut had_invalid_tail = false;

            if let Some(pad_result) = supports_fast_decode_and_encode.pad_remainder(&buffer) {
                had_invalid_tail = pad_result.had_invalid_tail;
                owned_chunk = Some(pad_result.chunk);
            }

            let final_chunk = owned_chunk.as_deref().unwrap_or(&buffer);

            supports_fast_decode_and_encode.decode_into_vec(final_chunk, &mut decoded_buffer)?;
            write_to_output(&mut decoded_buffer, output)?;

            if had_invalid_tail {
                return Err(SGSimpleError::new(1, "error: invalid input".to_owned()));
            }
        }

        Ok(())
    }
}

fn format_read_error(kind: ErrorKind) -> String {
    let kind_string = kind.to_string();

    let mut kind_string_capitalized = String::with_capacity(kind_string.len());

    for (index, ch) in kind_string.char_indices() {
        if index == 0 {
            for cha in ch.to_uppercase() {
                kind_string_capitalized.push(cha);
            }
        } else {
            kind_string_capitalized.push(ch);
        }
    }

    translate!("base-common-read-error", "error" => kind_string_capitalized)
}

#[cfg(test)]
mod tests {
    use crate::base_common::read_and_has_padding;
    use std::io::Cursor;

    #[test]
    fn test_has_padding() {
        let test_cases = vec![
            ("aGVsbG8sIHdvcmxkIQ==", true),
            ("aGVsbG8sIHdvcmxkIQ== ", true),
            ("aGVsbG8sIHdvcmxkIQ==\n", true),
            ("aGVsbG8sIHdvcmxkIQ== \n", true),
            ("aGVsbG8sIHdvcmxkIQ=", true),
            ("aGVsbG8sIHdvcmxkIQ= ", true),
            ("MTIzNA==MTIzNA", true),
            ("MTIzNA==\nMTIzNA", true),
            ("aGVsbG8sIHdvcmxkIQ \n", false),
            ("aGVsbG8sIHdvcmxkIQ", false),
        ];

        for (input, expected) in test_cases {
            let mut cursor = Cursor::new(input.as_bytes());
            assert_eq!(
                read_and_has_padding(&mut cursor).unwrap().0,
                expected,
                "Failed for input: '{input}'"
            );
        }
    }
}

