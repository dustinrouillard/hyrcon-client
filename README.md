# HyRCON Client

`hyrcon-client` is a Rust-powered command-line interface for the HyRCON remote console bridge. It speaks the plain-text TCP protocol implemented by `to.dstn.hytale.rcon.RconServer`, giving serverside admins a fast, scriptable alternative to manually attaching via `nc` or interactive tty consoles.

---

## Getting Started

### Prerequisites

- Rust toolchain (1.80+ recommended) with `cargo`.
- HyRCON RCON server (default port `5522`).

### Installation

#### Prebuilt binaries (Windows, macOS, Linux)

1. Download the latest release archive for your platform from https://github.com/dustinrouillard/hyrcon-client/releases/latest.
2. Extract the archive to a convenient location. On macOS and Linux mark the binary as executable:
```bash
chmod +x ./hyrcon-client
```
3. Run the binary directly from that directory (for example `./hyrcon-client` or `.\hyrcon-client.exe`), or move it to a directory on your `PATH` such as `/usr/local/bin`, `~/.local/bin`, or `%USERPROFILE%\.cargo\bin`.

#### macOS & Linux from source (Rustup + Cargo)

1. Install Rust via `rustup` if it isn't already available, then pull the CLI:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo install --locked --git https://github.com/dustinrouillard/hyrcon-client.git hyrcon-client
```
2. Ensure `~/.cargo/bin` is on your `PATH` so `hyrcon-client` is available in new shells (restart the terminal if `cargo` isn't found immediately).

#### Windows from source (PowerShell + Cargo)

1. Install Rust using the official `rustup` package, then install the CLI:
```powershell
winget install --id Rustlang.Rustup
cargo install --locked --git https://github.com/dustinrouillard/hyrcon-client.git hyrcon-client
```
2. Confirm `%USERPROFILE%\.cargo\bin` is on your `PATH`, if PowerShell still can't find `cargo`, open a new terminal session or run `& "$env:USERPROFILE\.cargo\env"`.

### Building from Source

```bash
git clone https://github.com/dustinrouillard/hyrcon-client.git
cd hyrcon-client

cargo build --release
```

The optimized binary will be placed at `target/release/hyrcon-client`.

### Running the CLI

```bash
# Execute a single command
cargo run -- --host 127.0.0.1 --port 5522 -- "say Hello from HyRCON"

# Start the interactive shell
cargo run -- --host 127.0.0.1 --port 5522
```

Flags & environment variables:

| Flag / Env            | Description                                        | Default        |
|-----------------------|----------------------------------------------------|----------------|
| `--host`, `RCON_HOST` | Server hostname/IP                                 | `127.0.0.1`    |
| `--port`, `RCON_PORT` | TCP port                                           | `5522`        |
| `--password`, `RCON_PASSWORD` | Password for `AUTH` handshake            | _none_         |
| `--timeout-ms`        | Read/write/connect timeout (milliseconds)          | `8000`         |
| `-v/--verbose`        | Increase log verbosity (repeat for TRACE)          | INFO level     |
| `--plain`             | Disable colorized output                           | false          |

Authentication notes:

- If the server advertises `AUTH REQUIRED`, you must provide a password (flag or env). The CLI exits with status `2` on auth failure.
- If `AUTH OPTIONAL` is reported, the CLI permits running commands without credentials but will attempt auth when a password is provided.

### Example Session

```text
$ hyrcon-client --host 127.0.0.1 --password secrets
⇢ HYRCON READY
Authentication required

rcon> PING
✔ OK PING
  PONG
```

Use `quit` or `exit` to close the session gracefully, EOF (`Ctrl+D`) will also terminate.
