// spell-checker:ignore (ToDO) getusername

#[cfg(unix)]
pub use self::unix::get_username;

#[cfg(unix)]
mod unix;

