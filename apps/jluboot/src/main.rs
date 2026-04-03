use clap::Parser;

fn main() {
    let cli = jluboot::Cli::parse();
    std::process::exit(jluboot::run_cli(cli));
}
