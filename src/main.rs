mod cli;
mod launch;
mod settings;
mod store;
mod ui;

fn main() {
    let args = cli::Args::new(std::env::args().skip(1).collect());
    if let Err(error) = cli::run(args) {
        eprintln!("herdr-inbox: {error}");
        std::process::exit(1);
    }
}
