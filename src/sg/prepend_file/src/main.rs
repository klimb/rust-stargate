use sg_prepend_file::uumain;

use std::io::Write;
use sgcore::error::UResult;

fn main() -> UResult<()> {
    sgcore::panic::install_sigpipe_hook();
    let result = uumain(sgcore::args_os());

    match result {
        Err(e) => {
            let s = format!("{e}");

            if !s.is_empty() {
                std::io::stderr().write_all(s.as_bytes()).unwrap();
                std::io::stderr().write_all(b"\n").unwrap();
            }

            std::process::exit(1);
        }
        Ok(()) => std::process::exit(0),
    }
}
