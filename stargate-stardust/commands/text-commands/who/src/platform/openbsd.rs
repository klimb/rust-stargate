

use crate::sg_app;

use sgcore::error::SGResult;
use sgcore::translate;

pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let _matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio"])?;
    println!("{}", translate!("who-unsupported-openbsd"));
    Ok(())
}

