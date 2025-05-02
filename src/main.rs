use anyhow::Result;
use clap::{Parser, Subcommand};
use log::info;

pub mod common;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as client (screen capture)
    Client {
        /// Server address to connect to
        #[arg(short, long, default_value = "127.0.0.1")]
        server: String,

        /// Server port
        #[arg(short, long, default_value_t = common::network::DEFAULT_PORT)]
        port: u16,

        /// Target frames per second
        #[arg(short, long, default_value_t = 30)]
        fps: u8,

        /// Quality (1-100)
        #[arg(short, long, default_value_t = 80)]
        quality: u8,
    },
    /// Run as server (display)
    Server {
        /// Bind address
        #[arg(short, long, default_value = "0.0.0.0")]
        bind: String,

        /// Bind port
        #[arg(short, long, default_value_t = common::network::DEFAULT_PORT)]
        port: u16,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logger
    let log_level = if cli.verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };
    
    env_logger::Builder::new()
        .filter_level(log_level)
        .init();

    info!("Cleverest v{} starting up", VERSION);

    match cli.command {
        #[cfg(feature = "client")]
        Commands::Client {
            server,
            port,
            fps,
            quality,
        } => {
            info!("Starting client, connecting to {}:{}", server, port);
            // In a real implementation, this would call the client module's run function
            // but for demo purposes we just show the command line usage
            info!("Using FPS: {}, Quality: {}", fps, quality);
            info!("To run the actual client binary, use: cargo run --bin client --features client");
            Ok(())
        }
        #[cfg(feature = "server")]
        Commands::Server { bind, port } => {
            info!("Starting server, listening on {}:{}", bind, port);
            // In a real implementation, this would call the server module's run function
            // but for demo purposes we just show the command line usage
            info!("To run the actual server binary, use: cargo run --bin server --features server");
            Ok(())
        }
        #[cfg(not(feature = "client"))]
        Commands::Client { .. } => {
            error!("Client mode not available in this build. Recompile with --features client");
            Ok(())
        }
        #[cfg(not(feature = "server"))]
        Commands::Server { .. } => {
            error!("Server mode not available in this build. Recompile with --features server");
            Ok(())
        }
    }
}