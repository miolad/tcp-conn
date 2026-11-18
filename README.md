# tcp-conn

A minimal TCP traffic generator that measures round-trip latency by bouncing a single byte between a client and a server.

## Build

Linux only (relies on Linux-specific socket flags).

```sh
cargo build --release
```

The binary is at `target/release/tcp-conn`.

## Usage

Start the server, then the client. Both sides must use the same `-n`, `-w` and `--per-conn` settings.

```sh
# Server (listens on 0.0.0.0:12345 by default)
tcp-conn -s [-b 0.0.0.0:12345]

# Client
tcp-conn -c <server-ip>:12345 > samples.txt
```

| Option | Description | Default |
|---|---|---|
| `-s` | Server mode | |
| `-c <ip:port>` | Client mode, connecting to the given server | |
| `-b <ip:port>` | Server bind address | `0.0.0.0:12345` |
| `-n <N>` | Number of measured iterations | `100000` |
| `-w <N>` | Warmup iterations (not reported) | `1000` |
| `--per-conn` | Open a new connection for every iteration instead of reusing one long-lived connection | off |

The client prints one round-trip time per line, in nanoseconds, to stdout.
