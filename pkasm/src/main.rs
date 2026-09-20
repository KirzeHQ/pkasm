use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "pkasm",
    version,
    about = "Assembly tooling and package management for PkASM",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Assemble an assembly source file.
    Assemble(FileCommand),
    /// Disassemble a program or binary file.
    Disassemble(FileCommand),
    /// Emulate a program.
    Emulate(FileCommand),
    /// Manage PkASM packages.
    Package {
        #[command(subcommand)]
        command: PackageCommand,
    },
}

#[derive(Debug, Args)]
struct FileCommand {
    /// Input file to process.
    input: PathBuf,
    /// Optional output file.
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum PackageCommand {
    /// Create a new PkASM package.
    Init {
        /// Directory in which to create the package.
        path: Option<PathBuf>,
    },
    /// Install a package.
    Install {
        /// Package to install.
        package: String,
    },
    /// List installed packages.
    List,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Assemble(command) => unavailable("assemble", command.input.display().to_string()),
        Command::Disassemble(command) => {
            unavailable("disassemble", command.input.display().to_string())
        }
        Command::Emulate(command) => unavailable("emulate", command.input.display().to_string()),
        Command::Package { command } => match command {
            PackageCommand::Init { path } => unavailable(
                "package init",
                path.map_or_else(|| ".".to_owned(), |path| path.display().to_string()),
            ),
            PackageCommand::Install { package } => unavailable("package install", package),
            PackageCommand::List => Err("package list is not implemented yet".to_owned()),
        },
    };

    if let Err(error) = result {
        eprintln!("pkasm: {error}");
        std::process::exit(1);
    }
}

fn unavailable(command: &str, target: String) -> Result<(), String> {
    Err(format!(
        "{command} is not implemented yet (target: {target})"
    ))
}
