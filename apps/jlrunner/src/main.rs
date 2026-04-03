use clap::Parser;

fn main() {
    let cli = jlrunner::Cli::parse();
    std::process::exit(jlrunner::run_cli(cli));
}
