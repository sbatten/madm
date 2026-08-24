use std::ffi::OsString;

fn main() {
    let args = std::env::args_os().skip(1).collect::<Vec<OsString>>();
    std::process::exit(madm::run(args));
}
