# Balebos

Balebos is a system control layer that keeps your infrastructure in a known, observable state.

It scans, normalizes and tracks assets (domains, services, repositories) and enforces basic operational invariants:
- everything is known
- everything is monitored
- nothing silently rots

Balebos is not just a dashboard.
It is not a SIEM.
It is a minimal control plane for reducing system entropy.

Balebos is designed so that system state does not depend on constant human attention.

## What it does

- discovers assets (domains, subdomains, repos)
- stores state in a simple, queryable form (JSONL / Parquet / DuckDB), optimized for low cost and ad-hoc analysis
- tracks changes over time
- exposes system state via API / queries
- highlights drift and unknowns

## Why

Most systems don’t fail loudly.
They decay:
- expired certificates
- forgotten services
- unmanaged domains
- silent data drift

Balebos exists to make that visible and controlled.

## Principles

- no magic
- deterministic pipelines
- simple storage over complex infrastructure
- visibility over abstraction
- cost-aware by default

## Status

Early stage. Core pipeline under active development.
