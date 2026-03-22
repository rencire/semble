use clap::Parser;

fn main() {
    let cli = semble::cli::Cli::parse();
    if let Err(error) = semble::run(cli) {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
