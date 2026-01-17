#![allow(dead_code)]

/// Utilities shared across the HYRCON client.
///
/// Currently this module provides helpers for normalising user input so that it
/// can be safely transmitted to the RCON server.
pub mod command {
  /// Sanitise raw user input before it is sent to the HYRCON server.
  ///
  /// The function trims trailing carriage-return (`\r`) and line-feed (`\n`)
  /// characters, then checks whether the remaining content is non-empty. An
  /// empty or whitespace-only input yields `None`, signalling that no command
  /// should be dispatched.
  ///
  /// # Examples
  ///
  /// ```
  /// use hyrcon_client::util::command::sanitize;
  ///
  /// assert_eq!(sanitize("say Hello\n"), Some("say Hello".to_string()));
  /// assert_eq!(sanitize("\n\n"), None);
  /// ```
  #[must_use]
  pub fn sanitize(raw: &str) -> Option<String> {
    let trimmed = raw.trim_matches(['\r', '\n']);
    if trimmed.trim().is_empty() {
      None
    } else {
      Some(trimmed.to_string())
    }
  }

  /// Determine whether the supplied command corresponds to a graceful exit.
  ///
  /// This helper recognises the built-in `quit` and `exit` verbs, ignoring
  /// ASCII case and leading/trailing whitespace.
  #[must_use]
  pub fn is_exit_command(raw: &str) -> bool {
    matches!(
      sanitize(raw)
        .as_deref()
        .map(|cmd| cmd.eq_ignore_ascii_case("quit")
          || cmd.eq_ignore_ascii_case("exit")),
      Some(true)
    )
  }
}

#[cfg(test)]
mod tests {
  use super::command::{is_exit_command, sanitize};

  #[test]
  fn sanitize_removes_trailing_newlines() {
    assert_eq!(sanitize("say hello\n"), Some("say hello".to_string()));
    assert_eq!(sanitize("say hello\r\n"), Some("say hello".to_string()));
    assert_eq!(sanitize("say hello\r\n\n"), Some("say hello".to_string()));
  }

  #[test]
  fn sanitize_rejects_blank_input() {
    assert_eq!(sanitize("   \n"), None);
    assert_eq!(sanitize("\n\n"), None);
  }

  #[test]
  fn exit_detection_is_case_insensitive() {
    assert!(is_exit_command("quit"));
    assert!(is_exit_command("QUIT"));
    assert!(is_exit_command(" Exit \n"));
    assert!(!is_exit_command("quiet"));
  }
}
