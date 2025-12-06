// Specific implementation for OpenBSD: tool unsupported (utmpx not supported)

use crate::sg_app;

use sgcore::error::UResult;
use sgcore::translate;

pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    let _matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    println!("{}", translate!("who-unsupported-openbsd"));
    Ok(())
}
