use owo_colors::OwoColorize;
use tokio::io::{self, AsyncWriteExt, Stdout};

use crate::transport::{Greeting, RconResponse, ResponseStatus};

/// Render the interactive prompt prefix to the provided stdout handle.
pub async fn render_prompt(
  stdout: &mut Stdout,
  use_color: bool,
) -> io::Result<()> {
  let prompt = if use_color {
    format!("{} ", "rcon>".bright_magenta().bold())
  } else {
    "rcon> ".to_owned()
  };

  stdout.write_all(prompt.as_bytes()).await?;
  stdout.flush().await
}

/// Pretty-print the server greeting block.
pub fn render_greeting(greeting: &Greeting, use_color: bool) {
  if use_color {
    println!("{} {}", "⇢".bright_cyan(), greeting.banner().bold());
  } else {
    println!("{}", greeting.banner());
  }

  let auth_message = match greeting.auth_mode() {
    crate::transport::AuthMode::Required => "Authentication required",
    crate::transport::AuthMode::Optional => "Authentication optional",
  };

  if use_color {
    match greeting.auth_mode() {
      crate::transport::AuthMode::Required => {
        println!("{}", auth_message.yellow().bold())
      }
      crate::transport::AuthMode::Optional => {
        println!("{}", auth_message.green().bold())
      }
    }
  } else {
    println!("{}", auth_message);
  }

  println!();
}

/// Render a command response in a human-friendly format.
pub fn render_response(
  command: &str,
  response: &RconResponse,
  use_color: bool,
) {
  let status_label = match response.status {
    ResponseStatus::Ok => {
      if use_color {
        format!("{}", "✔ OK".green().bold())
      } else {
        "OK".to_owned()
      }
    }
    ResponseStatus::Err => {
      if use_color {
        format!("{}", "✖ ERR".red().bold())
      } else {
        "ERR".to_owned()
      }
    }
  };

  println!("{status_label} {command}");

  for line in &response.payload {
    if use_color {
      println!("  {}", line.cyan());
    } else {
      println!("  {line}");
    }
  }

  if let Some(error) = &response.error {
    if use_color {
      println!("  {} {}", "⚠ ERROR".yellow().bold(), error.red().bold());
    } else {
      println!("  ERROR {error}");
    }
  }

  println!();
}

/// Show a farewell message when the server closes the session.
pub fn render_bye(use_color: bool) {
  if use_color {
    println!("{}", "⇢ Session closed by server".bright_magenta().bold());
  } else {
    println!("Session closed by server");
  }
}
