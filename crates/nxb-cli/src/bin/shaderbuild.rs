//! The `shaderbuild` binary.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = nxb_cli::run(
        nxb_cli::Binary::Shaderbuild,
        &args,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    );
    std::process::exit(code);
}
