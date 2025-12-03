// Specific implementation for OpenBSD: tool unsupported (utmpx not supported)

use crate::uu_app;

use sgcore::error::UResult;
use sgcore::translate;

pub fn uumain(args: impl sgcore::Args) -> UResult<()> {
    let _matches = sgcore::clap_localization::handle_clap_result(uu_app(), args)?;
    println!("{}", translate!("who-unsupported-openbsd"));
    Ok(())
}
