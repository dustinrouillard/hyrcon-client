# HyRCON Client

`hyrcon-client` is a Rust-powered command-line interface for the HyRCON remote console bridge. It speaks the plain-text TCP protocol implemented by `to.dstn.hytale.rcon.RconServer`, giving serverside admins a fast, scriptable alternative to manually attaching via `nc` or interactive tty consoles.

---

## Getting Started

### Prerequisites

- Rust toolchain (1.80+ recommended) with `cargo`.
- HyRCON RCON server (default port `5522`).

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

Use `quit` or `exit` to close the session gracefully; EOF (`Ctrl+D`) will also terminate.
