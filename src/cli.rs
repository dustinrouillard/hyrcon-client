use clap::{ArgAction, Parser};

/// Command-line arguments for the HYRCON client.
#[derive(Parser, Debug, Clone)]
#[command(
  author,
  version,
  about = "Interact with the HYRCON remote console bridge",
  trailing_var_arg = true
)]
pub struct Cli {
  /// Hostname or IP address of the HYRCON server.
  #[arg(long, env = "HYRCON_HOST", default_value = "127.0.0.1")]
  pub host: String,

  /// TCP port exposed by the HYRCON server.
  #[arg(long, env = "HYRCON_PORT", default_value_t = 5522)]
  pub port: u16,

  /// Password used for the AUTH handshake.
  #[arg(long, env = "HYRCON_PASSWORD")]
  pub password: Option<String>,

  /// I/O timeout in milliseconds.
  #[arg(long, default_value_t = 8_000, value_name = "MILLISECONDS")]
  pub timeout_ms: u64,

  /// Increase logging verbosity (repeat for TRACE).
  #[arg(short, long, action = ArgAction::Count)]
  pub verbose: u8,

  /// Disable ANSI color output.
  #[arg(long)]
  pub plain: bool,

  /// One-shot command executed instead of starting the REPL.
  #[arg(value_name = "COMMAND")]
  pub command: Vec<String>,
}
