# GAP Analysis — RECLI hardening (M0)

## current state

- cli supports start/stop/status/recent/clear but recent/clear are todo
- session management writes commands.json and session_metadata.json per session
- timestamps are local formatted strings; changed to rfc3339 utc in this milestone
- no canonical schema or validation existed
- no tests or ci in repo

## gaps identified

- missing schema and validation for logs (added schema:: and validate command)
- timestamps not rfc3339 (fixed, normalized in writer and validator helper)
- exit codes are guessed as 0 because shell integration is shallow
- command detection logic exists but not wired into logging_pty, output captured but mapping to events is simplistic
- config management absent (to be added in m1)
- telemetry/tracing absent (to be added in m1)
- no unit/integration tests yet (planned next milestones)

## decisions

- adopt LogEventV1 with rfc3339 utc timestamps and deterministic id using sha-256 of host|session|timestamp|command|offset
- for ingestion partitioning, recommend /session_id; document tradeoffs in schema doc
- keep existing file layout; will add config, util, ingest, and query modules incrementally per roadmap

## risks

- legacy sessions have non-rfc3339 timestamps; `recli validate` attempts to normalize `%Y-%m-%d %H:%M:%S` to rfc3339; others may fail and will be reported
- exit_code may be inaccurate; flag this in downstream analytics

## next

- m1: config loader and telemetry (tracing), retry util scaffolding
