use crate::{Cli, run};
use owo_colors::OwoColorize;

/// High-level wrapper that executes the HYRCON client lifecycle and reports errors uniformly.
pub struct Runtime {
  cli: Cli,
}

impl Runtime {
  /// Construct a new [`Runtime`] from parsed CLI arguments.
  #[must_use]
  pub fn new(cli: Cli) -> Self {
    Self { cli }
  }

  /// Execute the client and return the desired process exit code.
  ///
  /// On success the inner `run` function provides the exit status. Any error condition is logged
  /// in a colourful, human-friendly format and coerced to exit code `1`.
  pub async fn execute(self) -> i32 {
    match run(self.cli).await {
      Ok(code) => code,
      Err(err) => {
        log_error_chain(&err);
        1
      }
    }
  }
}

fn log_error_chain(err: &anyhow::Error) {
  eprintln!("{} {}", "error:".red().bold(), err.to_string().red().bold());

  for cause in err.chain().skip(1) {
    eprintln!("  {} {}", "↳".red(), cause);
  }
}
