use tracing_subscriber::EnvFilter;

/// Initialise structured logging for the HYRCON client.
///
/// `verbosity` comes from the CLI `-v/--verbose` flag:
///   * `0` → INFO
///   * `1` → DEBUG
///   * `2+` → TRACE
///
/// `use_color` controls whether ANSI colour codes are emitted.
pub fn init(verbosity: u8, use_color: bool) {
  // Map CLI verbosity to a tracing level.
  let level = match verbosity {
    0 => tracing::Level::INFO,
    1 => tracing::Level::DEBUG,
    _ => tracing::Level::TRACE,
  };

  // Respect `RUST_LOG` / `HYRCON_LOG` style environment overrides,
  // falling back to the computed base level.
  let filter = EnvFilter::try_from_default_env()
    .unwrap_or_else(|_| EnvFilter::new(level.as_str()));

  tracing_subscriber::fmt()
    .with_env_filter(filter)
    .with_target(false)
    .with_level(true)
    .with_ansi(use_color)
    .compact()
    .init();
}
