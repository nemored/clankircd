# clankircd
A next-generation IRCv3 server written in Rust.

Compatible with the Astarte IRC server. <http://irc.tejat.net>

See [`docs/implementation-plan.md`](docs/implementation-plan.md) for the implementation roadmap.

## Current status
Milestone 1 (IRC baseline) is implemented:
- IRC command parser/serializer now supports prefixes and trailing parameters.
- Registration flow supports `PASS` (optional), `NICK`, `USER`, and welcome numerics (`001`-`004`).
- Channel essentials are implemented: `JOIN`, `PART`, `PRIVMSG`, `NOTICE`, `QUIT`.
- CAP scaffold includes `CAP LS` and `CAP END` handling with `multi-prefix` advertisement.
- Structured logging with `tracing` and baseline in-process metrics snapshots remain enabled.

## Running locally
```bash
cargo run -p clankircd -- --config config/clankircd.toml
```

Then connect with a TCP client:
```bash
nc 127.0.0.1 6667
```
