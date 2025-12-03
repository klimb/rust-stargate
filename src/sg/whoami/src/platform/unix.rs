use std::ffi::OsString;
use std::io;

use sgcore::entries::uid2usr;
use sgcore::process::geteuid;

pub fn get_username() -> io::Result<OsString> {
    // uid2usr should arguably return an OsString but currently doesn't
    uid2usr(geteuid()).map(Into::into)
}
