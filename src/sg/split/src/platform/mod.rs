#[cfg(unix)]
pub use self::unix::instantiate_current_writer;
#[cfg(unix)]
pub use self::unix::paths_refer_to_same_file;

#[cfg(unix)]
mod unix;
