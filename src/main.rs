use anyhow::Context;

fn print_help() {
    println!(
        "\
kb {}

USAGE:
    kb [OPTIONS]

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information",
        env!("CARGO_PKG_VERSION")
    );
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    if let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-V" | "--version" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => {
                anyhow::bail!("unknown argument: {arg}. Use --help for usage information.");
            }
        }
    }

    kb::ui::run_ui().context("failed to run UI")?;
    Ok(())
}
