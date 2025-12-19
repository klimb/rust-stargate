fn main() {
    let result = <sg_monitor_spectrum::sgmain as sgcore::SGMain>::sgmain(sgcore::args_os());
    std::process::exit(result);
}
