# RECLI Canonical Log Schema (V1)

This document defines the canonical event schema used by RECLI for validation and cloud ingestion.

## LogEventV1

Fields:

- id: string — deterministic sha-256 of host|session_id|timestamp|command|offset
- schema_version: number — 1
- timestamp: string — RFC3339 UTC (e.g., 2025-09-07T12:34:56Z)
- host: string — hostname or machine id
- app: string — "recli"
- session_id: string — stable id for a session
- level: string — "INFO" | "WARN" | "ERROR"
- command: string — captured command line
- exit_code: number|null — exit status if known
- error_type: string|null — optional classification
- message: string — associated message or combined output
- tags: string[] — free-form labels
- raw: object|null — optional raw payload for provenance

## Partitioning

Cosmos recommendation: partition by /session_id for even distribution and natural query key. Trade-offs documented in the Azure proposal. Keep option to change in config when adding ingestion.

## Timestamps

All new events are written in RFC3339 UTC. The validator attempts to parse legacy `%Y-%m-%d %H:%M:%S` and normalize to RFC3339 during `recli validate`.

## Compatibility

- Older sessions may lack RFC3339 timestamps; these will be flagged by the validator if normalization fails.
- Future versions should bump `schema_version` and provide migration notes.
