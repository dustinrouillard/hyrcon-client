use std::io;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use tokio::io::{
  AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter,
};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::time::timeout as await_timeout;

/// Parsed greeting information returned by the HYRCON server.
#[derive(Debug, Clone)]
pub struct Greeting {
  banner: String,
  auth_mode: AuthMode,
}

impl Greeting {
  pub fn from_lines(lines: Vec<String>) -> Result<Self> {
    if lines.len() < 2 {
      bail!("protocol violation: greeting did not include auth mode");
    }

    let banner = lines.first().cloned().ok_or_else(|| {
      anyhow!("protocol violation: greeting missing banner")
    })?;

    if banner != "HYRCON READY" {
      bail!("unexpected greeting banner: {banner}");
    }

    let auth_mode = match lines[1].as_str() {
      "AUTH REQUIRED" => AuthMode::Required,
      "AUTH OPTIONAL" => AuthMode::Optional,
      other => {
        bail!("unknown authentication mode advertised by server: {other}")
      }
    };

    Ok(Self { banner, auth_mode })
  }

  pub fn requires_auth(&self) -> bool {
    matches!(self.auth_mode, AuthMode::Required)
  }

  pub fn banner(&self) -> &str {
    &self.banner
  }

  pub fn auth_mode(&self) -> AuthMode {
    self.auth_mode
  }
}

/// Indicates whether authentication is mandatory or optional.
#[derive(Debug, Clone, Copy)]
pub enum AuthMode {
  Required,
  Optional,
}

/// Result of issuing an AUTH command.
#[derive(Debug, Clone, Copy)]
pub enum AuthOutcome {
  Success,
  Failure,
}

/// Aggregated payload returned by the HYRCON server.
#[derive(Debug, Clone)]
pub struct RconResponse {
  pub status: ResponseStatus,
  pub payload: Vec<String>,
  pub error: Option<String>,
}

/// High-level status of a command response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseStatus {
  Ok,
  Err,
}

/// Possible outcomes when sending a protocol command.
#[derive(Debug)]
pub enum CommandOutcome {
  Response(RconResponse),
  Bye,
}

/// Client responsible for reading/writing the HYRCON wire protocol.
#[derive(Debug)]
pub struct RconClient {
  reader: BufReader<OwnedReadHalf>,
  writer: BufWriter<OwnedWriteHalf>,
  timeout: Duration,
  greeting: Greeting,
  closed: bool,
}

impl RconClient {
  /// Establish a connection, read the greeting block, and construct the client.
  pub async fn connect(
    host: &str,
    port: u16,
    deadline: Duration,
  ) -> Result<Self> {
    let stream = await_timeout(deadline, TcpStream::connect((host, port)))
      .await
      .context("connect timed out")?
      .context("connect failed")?;

    stream.set_nodelay(true)?;

    let (read_half, write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let greeting_lines = read_block(&mut reader, deadline)
      .await
      .context("failed to read greeting")?;
    let greeting = Greeting::from_lines(greeting_lines)?;

    Ok(Self {
      reader,
      writer: BufWriter::new(write_half),
      timeout: deadline,
      greeting,
      closed: false,
    })
  }

  pub fn greeting(&self) -> &Greeting {
    &self.greeting
  }

  pub fn is_closed(&self) -> bool {
    self.closed
  }

  /// Perform the AUTH handshake against the server.
  pub async fn authenticate(
    &mut self,
    password: &str,
  ) -> Result<AuthOutcome> {
    if password.contains(['\r', '\n']) {
      bail!("password must not contain newline characters");
    }

    self
      .write_line(&format!("AUTH {password}"), Some("AUTH <redacted>"))
      .await?;

    let block = read_block(&mut self.reader, self.timeout)
      .await
      .context("failed to read authentication response")?;

    match block.first().map(String::as_str) {
      Some("AUTH OK") => Ok(AuthOutcome::Success),
      Some("AUTH FAIL") => Ok(AuthOutcome::Failure),
      Some(other) => bail!("unexpected auth response: {other}"),
      None => bail!("server returned an empty block for AUTH response"),
    }
  }

  /// Send an arbitrary command line to the server.
  pub async fn send_command(
    &mut self,
    command: &str,
  ) -> Result<CommandOutcome> {
    if self.closed {
      bail!("connection already closed");
    }

    if command.trim().is_empty() {
      bail!("command must not be empty");
    }

    if command.contains(['\r', '\n']) {
      bail!("command must not contain newline characters");
    }

    self.write_line(command, Some(command)).await?;

    let block = read_block(&mut self.reader, self.timeout)
      .await
      .context("failed to read command response")?;

    let outcome = parse_command_block(block)?;
    if matches!(outcome, CommandOutcome::Bye) {
      self.closed = true;
    }

    Ok(outcome)
  }

  /// Issue QUIT and swallow any protocol errors during shutdown.
  pub async fn quit(&mut self) -> Result<()> {
    if self.closed {
      return Ok(());
    }

    match self.send_command("QUIT").await {
      Ok(CommandOutcome::Bye) => Ok(()),
      Ok(CommandOutcome::Response(response)) => {
        self.closed = true;
        bail!("unexpected payload in QUIT response: {:?}", response)
      }
      Err(err) => Err(err),
    }
  }

  async fn write_line(
    &mut self,
    line: &str,
    log_repr: Option<&str>,
  ) -> Result<()> {
    let label = log_repr.unwrap_or(line);
    tracing::debug!("--> {}", label);

    with_timeout(
      self.timeout,
      self.writer.write_all(line.as_bytes()),
      format!("writing `{label}` to socket"),
    )
    .await?;

    with_timeout(
      self.timeout,
      self.writer.write_all(b"\n"),
      format!("writing newline after `{label}`"),
    )
    .await?;

    with_timeout(
      self.timeout,
      self.writer.flush(),
      "flushing command to socket".to_string(),
    )
    .await?;

    Ok(())
  }
}

async fn with_timeout<F, T>(
  duration: Duration,
  future: F,
  context: impl Into<String>,
) -> Result<T>
where
  F: std::future::Future<Output = io::Result<T>>,
{
  let context = context.into();
  match await_timeout(duration, future).await {
    Ok(result) => result.with_context(|| context.clone()),
    Err(_) => Err(anyhow!(
      "{context} timed out after {} ms",
      duration.as_millis()
    )),
  }
}

async fn read_block<R>(
  reader: &mut R,
  duration: Duration,
) -> Result<Vec<String>>
where
  R: AsyncBufRead + Unpin,
{
  let mut lines = Vec::new();
  loop {
    let line = read_line(reader, duration).await?;
    if line == "." {
      break;
    }
    lines.push(line);
  }
  Ok(lines)
}

async fn read_line<R>(reader: &mut R, duration: Duration) -> Result<String>
where
  R: AsyncBufRead + Unpin,
{
  let mut buffer = String::new();
  let bytes_read = with_timeout(
    duration,
    reader.read_line(&mut buffer),
    "reading line from server",
  )
  .await?;

  if bytes_read == 0 {
    bail!("server closed the connection unexpectedly");
  }

  if buffer.ends_with('\n') {
    buffer.pop();
    if buffer.ends_with('\r') {
      buffer.pop();
    }
  }

  Ok(buffer)
}

fn parse_command_block(mut block: Vec<String>) -> Result<CommandOutcome> {
  if block.is_empty() {
    bail!("received empty response block from server");
  }

  let status_line = block.remove(0);
  match status_line.as_str() {
    "OK" => {
      let (payload, error) = extract_error(block);
      Ok(CommandOutcome::Response(RconResponse {
        status: ResponseStatus::Ok,
        payload,
        error,
      }))
    }
    "ERR" => {
      let (payload, error) = extract_error(block);
      Ok(CommandOutcome::Response(RconResponse {
        status: ResponseStatus::Err,
        payload,
        error,
      }))
    }
    "BYE" => Ok(CommandOutcome::Bye),
    other => bail!("unexpected status line `{other}` in command response"),
  }
}

fn extract_error(mut lines: Vec<String>) -> (Vec<String>, Option<String>) {
  if let Some(message) = lines
    .last()
    .and_then(|last| last.strip_prefix("ERROR ").map(String::from))
  {
    lines.pop();
    return (lines, Some(message));
  }
  (lines, None)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn greeting_parses_required_auth() {
    let greeting = Greeting::from_lines(vec![
      "HYRCON READY".to_string(),
      "AUTH REQUIRED".to_string(),
    ])
    .expect("parse greeting");

    assert!(greeting.requires_auth());
    assert_eq!(greeting.banner(), "HYRCON READY");
  }

  #[tokio::test]
  async fn extract_error_splits_last_line() {
    let (payload, error) = extract_error(vec![
      "line 1".to_string(),
      "ERROR Something went wrong".to_string(),
    ]);

    assert_eq!(payload, vec!["line 1"]);
    assert_eq!(error, Some("Something went wrong".to_string()));
  }
}
