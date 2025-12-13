use std::fs;
use std::path::Path;
use sgcore::mode;
use sgcore::translate;

/// Takes a user-supplied string and tries to parse to u16 mode bitmask.
pub fn parse(mode_string: &str, considering_dir: bool, umask: u32) -> Result<u32, String> {
    if mode_string.chars().any(|c| c.is_ascii_digit()) {
        mode::parse_numeric(0, mode_string, considering_dir)
    } else {
        mode::parse_symbolic(0, mode_string, umask, considering_dir)
    }
}

/// chmod a file or directory on UNIX.
///
/// Adapted from mkdir.rs.  Handles own error printing.
///
pub fn chmod(path: &Path, mode: u32) -> Result<(), ()> {
    use std::os::unix::fs::PermissionsExt;
    use sgcore::{display::Quotable, show_error};
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|err| {
        show_error!(
            "{}",
            translate!("install-error-chmod-failed-detailed", "path" => path.maybe_quote(), "error" => err)
        );
    })
}