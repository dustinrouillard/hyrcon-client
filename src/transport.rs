use std::io::{self, ErrorKind};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use tokio::io::{
  AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader,
  BufWriter,
};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::time::timeout as await_timeout;

use crate::protocol::Protocol;

/// Parsed greeting information returned (or synthesized) for the connected server.
#[derive(Debug, Clone)]
pub struct Greeting {
  banner: String,
  auth_mode: AuthMode,
  protocol: Protocol,
}

impl Greeting {
  fn new(
    protocol: Protocol,
    banner: impl Into<String>,
    auth_mode: AuthMode,
  ) -> Self {
    Self {
      banner: banner.into(),
      auth_mode,
      protocol,
    }
  }

  fn hyrcon_from_lines(lines: Vec<String>) -> Result<Self> {
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

    Ok(Self::new(Protocol::Hyrcon, banner, auth_mode))
  }

  pub fn from_lines(lines: Vec<String>) -> Result<Self> {
    Self::hyrcon_from_lines(lines)
  }

  pub fn source_default() -> Self {
    Self::new(Protocol::Source, "SOURCE RCON READY", AuthMode::Required)
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

  pub fn protocol(&self) -> Protocol {
    self.protocol
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

/// Aggregated payload returned by the RCON server.
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

/// Client responsible for reading/writing the selected RCON wire protocol.
#[derive(Debug)]
pub struct RconClient {
  backend: Backend,
  greeting: Greeting,
  protocol: Protocol,
}

#[derive(Debug)]
enum Backend {
  Hyrcon(HyrconClient),
  Source(SourceClient),
}

impl RconClient {
  /// Establish a connection for the given protocol and construct the client.
  pub async fn connect(
    protocol: Protocol,
    host: &str,
    port: u16,
    deadline: Duration,
  ) -> Result<Self> {
    match protocol {
      Protocol::Hyrcon => {
        let (client, greeting) =
          HyrconClient::connect(host, port, deadline).await?;
        Ok(Self {
          backend: Backend::Hyrcon(client),
          greeting,
          protocol,
        })
      }
      Protocol::Source => {
        let client = SourceClient::connect(host, port, deadline).await?;
        let greeting = Greeting::source_default();
        Ok(Self {
          backend: Backend::Source(client),
          greeting,
          protocol,
        })
      }
    }
  }

  pub fn protocol(&self) -> Protocol {
    self.protocol
  }

  pub fn greeting(&self) -> &Greeting {
    &self.greeting
  }

  pub fn is_closed(&self) -> bool {
    match &self.backend {
      Backend::Hyrcon(client) => client.is_closed(),
      Backend::Source(client) => client.is_closed(),
    }
  }

  /// Perform the authentication handshake as required by the backend.
  pub async fn authenticate(
    &mut self,
    password: &str,
  ) -> Result<AuthOutcome> {
    match &mut self.backend {
      Backend::Hyrcon(client) => client.authenticate(password).await,
      Backend::Source(client) => client.authenticate(password).await,
    }
  }

  /// Send an arbitrary command line to the server.
  pub async fn send_command(
    &mut self,
    command: &str,
  ) -> Result<CommandOutcome> {
    match &mut self.backend {
      Backend::Hyrcon(client) => client.send_command(command).await,
      Backend::Source(client) => client.send_command(command).await,
    }
  }

  /// Attempt a graceful shutdown of the session.
  pub async fn quit(&mut self) -> Result<()> {
    match &mut self.backend {
      Backend::Hyrcon(client) => client.quit().await,
      Backend::Source(client) => client.quit().await,
    }
  }
}

#[derive(Debug)]
struct HyrconClient {
  reader: BufReader<OwnedReadHalf>,
  writer: BufWriter<OwnedWriteHalf>,
  timeout: Duration,
  closed: bool,
}

impl HyrconClient {
  async fn connect(
    host: &str,
    port: u16,
    deadline: Duration,
  ) -> Result<(Self, Greeting)> {
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
    let greeting = Greeting::hyrcon_from_lines(greeting_lines)?;

    Ok((
      Self {
        reader,
        writer: BufWriter::new(write_half),
        timeout: deadline,
        closed: false,
      },
      greeting,
    ))
  }

  fn is_closed(&self) -> bool {
    self.closed
  }

  async fn authenticate(&mut self, password: &str) -> Result<AuthOutcome> {
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

  async fn send_command(
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

  async fn quit(&mut self) -> Result<()> {
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

#[derive(Debug)]
struct SourceClient {
  reader: BufReader<OwnedReadHalf>,
  writer: BufWriter<OwnedWriteHalf>,
  timeout: Duration,
  authed: bool,
  next_request_id: i32,
  closed: bool,
}

const SERVERDATA_RESPONSE_VALUE: i32 = 0;
const SERVERDATA_EXECCOMMAND: i32 = 2;
const SERVERDATA_AUTH_RESPONSE: i32 = 2;
const SERVERDATA_AUTH: i32 = 3;

impl SourceClient {
  async fn connect(
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

    Ok(Self {
      reader: BufReader::new(read_half),
      writer: BufWriter::new(write_half),
      timeout: deadline,
      authed: false,
      next_request_id: 1,
      closed: false,
    })
  }

  fn is_closed(&self) -> bool {
    self.closed
  }

  fn next_request_id(&mut self) -> i32 {
    let id = self.next_request_id;
    self.next_request_id = self.next_request_id.wrapping_add(1);
    id
  }

  async fn authenticate(&mut self, password: &str) -> Result<AuthOutcome> {
    if password.contains(['\r', '\n']) {
      bail!("password must not contain newline characters");
    }
    if password.contains('\0') {
      bail!("password must not contain NUL characters");
    }

    let auth_id = self.next_request_id();
    self
      .write_packet(
        auth_id,
        SERVERDATA_AUTH,
        password,
        Some("AUTH <redacted>"),
      )
      .await?;

    let mut outcome = AuthOutcome::Failure;
    loop {
      let packet = self.read_packet().await?;
      match packet.kind {
        SERVERDATA_RESPONSE_VALUE => {
          // Ignore intermediary response-value packet emitted by some servers.
          continue;
        }
        SERVERDATA_AUTH_RESPONSE => {
          if packet.id == auth_id {
            self.authed = true;
            outcome = AuthOutcome::Success;
          } else if packet.id == -1 {
            self.authed = false;
            outcome = AuthOutcome::Failure;
          } else {
            tracing::debug!(
              response_id = packet.id,
              expected_id = auth_id,
              "received unexpected AUTH response identifier"
            );
          }
          break;
        }
        other => {
          tracing::debug!(
            packet_id = packet.id,
            packet_kind = other,
            "ignoring unexpected packet while authenticating"
          );
        }
      }
    }

    Ok(outcome)
  }

  async fn send_command(
    &mut self,
    command: &str,
  ) -> Result<CommandOutcome> {
    if self.closed {
      bail!("connection already closed");
    }

    if !self.authed {
      bail!("server requires authentication before sending commands");
    }

    if command.trim().is_empty() {
      bail!("command must not be empty");
    }

    if command.contains(['\r', '\n']) {
      bail!("command must not contain newline characters");
    }

    if command.contains('\0') {
      bail!("command must not contain NUL characters");
    }

    let command_id = self.next_request_id();
    tracing::debug!(request_id = command_id, "--> {}", command);
    self
      .write_packet(
        command_id,
        SERVERDATA_EXECCOMMAND,
        command,
        Some(command),
      )
      .await?;

    // Sentinel packet to delimit the end of the response stream.
    let sentinel_id = self.next_request_id();
    self
      .write_packet(
        sentinel_id,
        SERVERDATA_EXECCOMMAND,
        "",
        Some("<sentinel>"),
      )
      .await?;

    let mut payload_lines = Vec::new();

    loop {
      let packet = self.read_packet().await?;

      if packet.kind == SERVERDATA_AUTH_RESPONSE && packet.id == -1 {
        self.authed = false;
        bail!("server reported that authentication is no longer valid");
      }

      if packet.id == sentinel_id {
        if packet.kind != SERVERDATA_RESPONSE_VALUE {
          bail!(
            "server returned unexpected sentinel packet kind: {}",
            packet.kind
          );
        }
        if !packet.payload.is_empty() {
          bail!("server returned data alongside sentinel response");
        }
        break;
      }

      if packet.kind == SERVERDATA_RESPONSE_VALUE
        && packet.id == command_id
      {
        if !packet.payload.is_empty() {
          payload_lines.extend(split_lines(&packet.payload));
        }
        continue;
      }

      tracing::debug!(
        packet_id = packet.id,
        packet_kind = packet.kind,
        "ignoring non-matching packet while collecting response"
      );
    }

    Ok(CommandOutcome::Response(RconResponse {
      status: ResponseStatus::Ok,
      payload: payload_lines,
      error: None,
    }))
  }

  async fn quit(&mut self) -> Result<()> {
    if self.closed {
      return Ok(());
    }

    self.closed = true;

    with_timeout(
      self.timeout,
      self.writer.flush(),
      "flushing buffered data before shutdown".to_string(),
    )
    .await?;

    with_timeout(
      self.timeout,
      self.writer.shutdown(),
      "shutting down Source RCON writer".to_string(),
    )
    .await?;

    Ok(())
  }

  async fn write_packet(
    &mut self,
    id: i32,
    kind: i32,
    payload: &str,
    log_repr: Option<&str>,
  ) -> Result<()> {
    let label = log_repr.unwrap_or(payload);
    tracing::trace!(
      request_id = id,
      packet_kind = kind,
      "writing packet {label}"
    );

    if payload.contains('\0') {
      bail!("payloads must not contain NUL characters");
    }

    let payload_bytes = payload.as_bytes();
    let length = 4 + 4 + payload_bytes.len() + 2;
    let length_bytes = (length as i32).to_le_bytes();
    let mut packet = Vec::with_capacity(4 + length);

    packet.extend_from_slice(&length_bytes);
    packet.extend_from_slice(&id.to_le_bytes());
    packet.extend_from_slice(&kind.to_le_bytes());
    packet.extend_from_slice(payload_bytes);
    packet.push(0);
    packet.push(0);

    with_timeout(
      self.timeout,
      self.writer.write_all(&packet),
      format!("writing `{label}` packet to socket"),
    )
    .await?;

    with_timeout(
      self.timeout,
      self.writer.flush(),
      format!("flushing `{label}` packet to socket"),
    )
    .await?;

    Ok(())
  }

  async fn read_packet(&mut self) -> Result<SourcePacket> {
    let mut length_bytes = [0_u8; 4];
    if let Err(err) = with_timeout(
      self.timeout,
      self.reader.read_exact(&mut length_bytes),
      "reading packet length from Source RCON server".to_string(),
    )
    .await
    {
      if is_unexpected_eof(&err) {
        self.closed = true;
      }
      return Err(err);
    }

    let length = i32::from_le_bytes(length_bytes);
    if length < 10 {
      bail!(
        "Source RCON packet reported invalid payload length: {length}"
      );
    }

    let mut buffer = vec![0_u8; length as usize];
    if let Err(err) = with_timeout(
      self.timeout,
      self.reader.read_exact(&mut buffer),
      "reading Source RCON packet payload".to_string(),
    )
    .await
    {
      if is_unexpected_eof(&err) {
        self.closed = true;
      }
      return Err(err);
    }

    let mut id_bytes = [0_u8; 4];
    id_bytes.copy_from_slice(&buffer[0..4]);
    let id = i32::from_le_bytes(id_bytes);
    let mut kind_bytes = [0_u8; 4];
    kind_bytes.copy_from_slice(&buffer[4..8]);
    let kind = i32::from_le_bytes(kind_bytes);

    if buffer.len() < 10 {
      bail!("Source RCON packet too small after header decoding");
    }

    if buffer[buffer.len() - 2] != 0 || buffer[buffer.len() - 1] != 0 {
      bail!("Source RCON packet missing trailing NUL terminators");
    }

    let payload_bytes = &buffer[8..buffer.len() - 2];
    let payload_raw =
      String::from_utf8(payload_bytes.to_vec()).map_err(|err| {
        anyhow!("received non-UTF8 data in Source RCON packet: {err}")
      })?;
    let payload = payload_raw.split('\0').next().unwrap_or("").to_string();

    tracing::trace!(
      packet_id = id,
      packet_kind = kind,
      payload_len = payload_raw.len(),
      "received Source RCON packet"
    );

    Ok(SourcePacket { id, kind, payload })
  }
}

#[derive(Debug)]
struct SourcePacket {
  id: i32,
  kind: i32,
  payload: String,
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
    "reading line from server".to_string(),
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

fn split_lines(payload: &str) -> Vec<String> {
  if payload.is_empty() {
    return vec![];
  }

  payload
    .lines()
    .map(|line| line.trim_end_matches('\r').to_string())
    .collect()
}

fn is_unexpected_eof(err: &anyhow::Error) -> bool {
  err
    .downcast_ref::<io::Error>()
    .map(|io_err| io_err.kind() == ErrorKind::UnexpectedEof)
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn greeting_parses_required_auth() {
    let greeting = Greeting::hyrcon_from_lines(vec![
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

  #[test]
  fn split_lines_handles_crlf() {
    let lines = split_lines("foo\r\nbar\nbaz\r\n");
    assert_eq!(lines, vec!["foo", "bar", "baz"]);
  }
}
