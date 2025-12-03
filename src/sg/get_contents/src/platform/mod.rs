#[cfg(unix)]
pub use self::unix::is_unsafe_overwrite;

#[cfg(unix)]
mod unix;
