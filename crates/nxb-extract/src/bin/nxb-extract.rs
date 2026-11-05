//! `nxb-extract`: the extract command line (`nxb_extract::cli`), the same as `nxb extract`.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(nxb_extract::cli::main(&args));
}
