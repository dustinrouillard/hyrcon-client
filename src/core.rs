use std::io::{self, IsTerminal};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::{
  cli::Cli,
  logging,
  transport::{
    self, AuthOutcome, CommandOutcome, RconClient, ResponseStatus,
  },
  ui,
  util::command,
};

/// Orchestrate the full HYRCON client lifecycle for a single invocation.
pub async fn run(cli: Cli) -> Result<i32> {
  let use_color_stdout = !cli.plain && io::stdout().is_terminal();
  let use_color_logs = !cli.plain && io::stderr().is_terminal();

  logging::init(cli.verbose, use_color_logs);

  let mut client = transport::RconClient::connect(
    &cli.host,
    cli.port,
    Duration::from_millis(cli.timeout_ms),
  )
  .await
  .with_context(|| {
    format!("failed to connect to {}:{}", cli.host, cli.port)
  })?;

  let greeting = client.greeting().clone();
  tracing::info!(
    auth_required = greeting.requires_auth(),
    banner = greeting.banner(),
    "connected to HYRCON server"
  );
  ui::render_greeting(&greeting, use_color_stdout);

  authenticate_if_required(&cli, &mut client).await?;

  let exit_code = if cli.command.is_empty() {
    run_interactive(&mut client, use_color_stdout).await?
  } else {
    run_one_shot(&cli, &mut client, use_color_stdout).await?
  };

  if !client.is_closed() {
    if let Err(err) = client.quit().await {
      tracing::debug!(error = %err, "failed to send QUIT during shutdown");
    }
  }

  Ok(exit_code)
}

async fn authenticate_if_required(
  cli: &Cli,
  client: &mut RconClient,
) -> Result<()> {
  if client.greeting().requires_auth() {
    let password = cli
            .password
            .as_deref()
            .ok_or_else(|| anyhow!("server requires authentication; supply --password or set RCON_PASSWORD"))?;

    match client.authenticate(password).await? {
      AuthOutcome::Success => tracing::info!("authentication accepted"),
      AuthOutcome::Failure => bail!("authentication rejected by server"),
    }
  } else if let Some(password) = cli.password.as_deref() {
    match client.authenticate(password).await? {
      AuthOutcome::Success => tracing::info!("authenticated (optional)"),
      AuthOutcome::Failure => tracing::warn!(
        "authentication failed but server allows unauthenticated commands; continuing without credentials"
      ),
    }
  }

  Ok(())
}

async fn run_one_shot(
  cli: &Cli,
  client: &mut RconClient,
  use_color: bool,
) -> Result<i32> {
  let command_text = cli.command.join(" ");
  let command = command::sanitize(&command_text).ok_or_else(|| {
    anyhow!("command was empty after trimming whitespace")
  })?;

  match client.send_command(&command).await? {
    CommandOutcome::Response(response) => {
      ui::render_response(&command, &response, use_color);
      if matches!(response.status, ResponseStatus::Err) {
        Ok(2)
      } else {
        Ok(0)
      }
    }
    CommandOutcome::Bye => {
      ui::render_bye(use_color);
      Ok(0)
    }
  }
}

async fn run_interactive(
  client: &mut RconClient,
  use_color: bool,
) -> Result<i32> {
  let mut stdin = BufReader::new(tokio::io::stdin());
  let mut stdout = tokio::io::stdout();
  let mut input = String::new();
  let mut exit_code = 0;

  loop {
    ui::render_prompt(&mut stdout, use_color)
      .await
      .context("failed to render prompt")?;

    input.clear();
    let bytes_read = stdin
      .read_line(&mut input)
      .await
      .context("failed to read line from stdin")?;

    if bytes_read == 0 {
      println!();
      tracing::info!("stdin closed; terminating session");
      break;
    }

    let Some(command) = command::sanitize(&input) else {
      continue;
    };

    let exit_command = command::is_exit_command(&input);

    match client.send_command(&command).await? {
      CommandOutcome::Response(response) => {
        ui::render_response(&command, &response, use_color);
        if matches!(response.status, ResponseStatus::Err) {
          exit_code = 2;
        }
        if exit_command {
          break;
        }
      }
      CommandOutcome::Bye => {
        ui::render_bye(use_color);
        break;
      }
    }
  }

  Ok(exit_code)
}
