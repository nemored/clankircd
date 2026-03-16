# ClankyIRCd Implementation Plan

This document outlines a practical delivery plan for building **clankircd**, a next-generation IRCv3 server in Rust with hot-reload support.

## 1. Goals and Scope

### Core goals
- Build a production-ready IRC server in Rust.
- Prioritize IRCv3 protocol compliance and interoperability.
- Support safe hot-reloading for selected runtime configuration.
- Define a hot code reloading design requirement that allows operator-initiated zero-downtime binary swaps for compatible releases.
- Keep the architecture modular for future extensions (services, clustering, plugins).

### Non-goals (initial release)
- Full services suite (NickServ/ChanServ equivalents).
- Cross-node federation/clustering.
- Dynamic third-party plugin loading.

## 2. Milestones

## Milestone 0: Project foundation (1 week)
- Create crate layout:
  - `crates/protocol` (IRC parsing/serialization)
  - `crates/server-core` (state machine + routing)
  - `crates/transport` (TCP/TLS listeners)
  - `crates/config` (schema + loader + validation)
  - `bin/clankircd` (runtime entrypoint)
- Setup CI (format, clippy, unit tests).
- Add structured logging/tracing and baseline metrics.

**Exit criteria**
- Binary starts, binds to configured port, accepts raw TCP connections.

## Milestone 1: IRC baseline (2 weeks)
- Implement command parser and serializer for core RFC-style commands.
- Implement connection registration flow:
  - `PASS` (optional)
  - `NICK`
  - `USER`
  - welcome numerics (`001`+ minimal set)
- Implement channel model and essentials:
  - `JOIN`, `PART`, `PRIVMSG`, `NOTICE`, `QUIT`
- Add server capability advertisement scaffold.

**Exit criteria**
- Users can connect via standard IRC clients and chat in channels.

## Milestone 2: IRCv3 capability set (2–3 weeks)
- Implement CAP negotiation state machine:
  - `CAP LS`, `CAP REQ`, `CAP ACK/NAK`, `CAP END`
- First IRCv3 capabilities:
  - `message-tags`
  - `server-time`
  - `multi-prefix`
  - `away-notify`
  - `account-notify` (stub if no auth service yet)
- Add feature compatibility test matrix with common clients.

**Exit criteria**
- Successful capability negotiation with at least two IRCv3-capable clients.

## Milestone 3: Hot-reload architecture (1–2 weeks)
- Define immutable vs mutable config domains.
  - Mutable examples: MOTD, channel defaults, rate limits.
  - Immutable examples: listener sockets, TLS key material (initially).
- Implement file watcher + explicit reload signal (`SIGHUP` and/or admin command).
- Implement atomic config swap with validation-before-apply.
- Emit operator-visible reload success/failure events.

**Exit criteria**
- Valid config reload applies at runtime without dropping active sessions.

## Milestone 3.5: Hot code reload design requirement (1 week)
- Publish an architecture decision record (ADR) for hot code reload behavior.
- Define compatibility contract for reloadable binaries (protocol/state schema, session handoff invariants).
- Specify process model for handoff (`exec`-style re-exec or sidecar handover), including rollback path.
- Define observability and operator UX requirements (pre-flight checks, progress logging, abort semantics).
- Add non-goals for the first design iteration (e.g., no live plugin ABI loading).

**Exit criteria**
- Approved design spec exists with clear implementation boundaries, risk analysis, and test strategy for future milestones.

## Milestone 4: Security and reliability hardening (2 weeks)
- Rate limiting and flood controls.
- Connection/account throttles and banline framework.
- TLS listener support and modern cipher defaults.
- Panic isolation strategy and graceful shutdown path.

**Exit criteria**
- Fuzzing and stress tests complete without critical crashes.

## Milestone 5: Packaging and operations (1 week)
- Container image + minimal runtime config template.
- Example systemd unit and operational runbook.
- Health endpoints/metrics export for observability.

**Exit criteria**
- Reproducible deployment in staging environment.

## 3. Cross-cutting Engineering Work

- **Testing strategy**
  - Unit tests for parser/state transitions.
  - Integration tests for registration, channel routing, and CAP.
  - Protocol transcript tests for deterministic responses.
- **Performance**
  - Benchmark parser throughput and fan-out paths.
  - Track memory per-connection and per-channel.
- **Developer experience**
  - `just` or `make` targets for common tasks.
  - Local dev config and scripted smoke tests.

## 4. Risks and Mitigations

- **Protocol edge cases**: Use transcript-based regression tests and compatibility fixtures.
- **Reload safety**: Apply staged validation and rollback on failed swap.
- **Feature creep**: Gate non-core features behind RFC/issue tracking and milestone boundaries.

## 5. Definition of Done (v0.1)

- Stable single-node IRC server with core IRCv3 capabilities.
- Hot-reload for designated mutable configuration.
- CI enforcing formatting, linting, and test pass.
- Operator documentation for startup, reload, and troubleshooting.
