# clankircd
A next-generation IRCv3 server written in Rust.

Compatible with the Astarte IRC server. <http://irc.tejat.net>

See [`docs/implementation-plan.md`](docs/implementation-plan.md) for the implementation roadmap.

## Current status
Milestone 0 (project foundation) scaffold is in place:
- Rust workspace with protocol, server-core, transport, config crates, and `clankircd` binary.
- Structured logging with `tracing`.
- Baseline in-process metrics snapshots emitted at a configurable interval.
- CI workflow for formatting, linting, and tests.

## Running locally
```bash
cargo run -p clankircd -- --config config/clankircd.toml
```

Then connect with a TCP client:
```bash
nc 127.0.0.1 6667
```
