use sgcore::error::{set_exit_code, UResult};
use std::io::Write;
use sg_list_processes::{uu_app, uumain};

#[sgcore::main]
fn main() -> UResult<()> {
    let result = uumain(sgcore::args_os());
    if let Err(e) = &result {
        let s = format!("{}", e);
        if s != "" {
            std::io::stderr().write_all(s.as_bytes()).unwrap();
            std::io::stderr().write_all(b"\n").unwrap();
        }
        set_exit_code(e.code());
    }
    result
}
