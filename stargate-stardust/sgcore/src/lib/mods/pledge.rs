#[cfg(target_os = "openbsd")]
pub use pledge::pledge_promises;

#[cfg(target_os = "openbsd")]
pub fn apply_pledge(promises: &[&str]) -> crate::error::SGResult<()> {
    use crate::error::SGSimpleError;
    let promise_str = promises.join(" ");

    pledge::pledge(&[promise_str.as_str()], None).map_err(|e| {
        SGSimpleError::new(1, format!("pledge failed: {}", e))
    })
}

#[cfg(not(target_os = "openbsd"))]
pub fn apply_pledge(_promises: &[&str]) -> crate::error::SGResult<()> {
    Ok(())
}

