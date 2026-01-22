use std::fmt;
use std::str::FromStr;

/// Supported RCON wire protocols.
///
/// `Protocol::Source` is the default and represents the Valve/Source RCON
/// dialect. `Protocol::Hyrcon` corresponds to the legacy HYRCON bridge
/// protocol used by older servers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
  /// Valve/Source RCON protocol.
  Source,
  /// Legacy HYRCON bridge protocol.
  Hyrcon,
}

impl Protocol {
  /// Returns the canonical lowercase string representation of the protocol.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Source => "source",
      Self::Hyrcon => "hyrcon",
    }
  }

  /// Returns the default TCP port typically used by the protocol.
  pub const fn default_port(self) -> u16 {
    match self {
      Self::Source => 25_575,
      Self::Hyrcon => 5_522,
    }
  }
}

impl Default for Protocol {
  fn default() -> Self {
    Self::Source
  }
}

impl fmt::Display for Protocol {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

/// Error returned when parsing a [`Protocol`] from text fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseProtocolError {
  input: String,
}

impl ParseProtocolError {
  /// Creates a new parse error capturing the offending input.
  pub fn new(input: impl Into<String>) -> Self {
    Self {
      input: input.into(),
    }
  }

  /// Returns the original input that failed to parse.
  pub fn input(&self) -> &str {
    &self.input
  }
}

impl fmt::Display for ParseProtocolError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "unsupported protocol `{}`", self.input)
  }
}

impl std::error::Error for ParseProtocolError {}

impl FromStr for Protocol {
  type Err = ParseProtocolError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let normalized = s.trim().to_ascii_lowercase();
    match normalized.as_str() {
      "source" | "src" => Ok(Self::Source),
      "hyrcon" | "legacy" => Ok(Self::Hyrcon),
      _ => Err(ParseProtocolError::new(s)),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_is_source() {
    assert_eq!(Protocol::default(), Protocol::Source);
  }

  #[test]
  fn default_ports_match_expectations() {
    assert_eq!(Protocol::Source.default_port(), 25_575);
    assert_eq!(Protocol::Hyrcon.default_port(), 5_522);
  }

  #[test]
  fn parse_accepts_common_aliases() {
    assert_eq!("source".parse::<Protocol>(), Ok(Protocol::Source));
    assert_eq!("SRC".parse::<Protocol>(), Ok(Protocol::Source));
    assert_eq!("hyrcon".parse::<Protocol>(), Ok(Protocol::Hyrcon));
    assert_eq!("LEGACY".parse::<Protocol>(), Ok(Protocol::Hyrcon));
  }

  #[test]
  fn parse_rejects_unknown_values() {
    let err = "minecraft".parse::<Protocol>().unwrap_err();
    assert_eq!(err.input(), "minecraft");
    assert_eq!(err.to_string(), "unsupported protocol `minecraft`");
  }
}
