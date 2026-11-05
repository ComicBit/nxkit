//! The `airc` program: the command line is [`airc::main`].

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(airc::main(&args));
}
