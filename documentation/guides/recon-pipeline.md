# Reconnaissance Pipeline Guide

This guide explains how Balebos orchestrates external tools to build a comprehensive inventory of your infrastructure.

## The Standard Workflow

Balebos follows a 5-step pipeline designed to maximize visibility while minimizing redundant operations.

### Step 1: Scan & Normalize (`balebos scan`)
The process starts by adding root domains to the inventory. 
- Input is validated for structural correctness (TLD presence).
- Input is normalized (lowercase, protocol stripped, www stripped).
- A base `domain` record is created.

### Step 2: Registration Data (`rdap`)
For every root domain, Balebos fetches registration information using the RDAP protocol.
- Fetches **Registrar** name and **IANA ID**.
- Records **Registration**, **Expiration**, and **Last Update** dates.
- Extracts contact information (**Registrant**, **Admin**, **Tech**) where available.
- **Fallback**: Automatically falls back to traditional WHOIS if RDAP is not supported for a TLD.

### Step 3: Subdomain Discovery (`subfinder`)
Balebos invokes `subfinder` to discover all known subdomains associated with the root domain.
- Uses passive sources (APIs, archives).
- Discovered subdomains are added to the inventory as `subdomain` records.

### Step 4: DNS Resolution (`dnsx`)
For every domain and subdomain in the inventory, Balebos runs `dnsx`.
- Verifies if the host is alive at the network level.
- Records `A`, `AAAA`, `CNAME`, and `MX` records.
- This step acts as a filter: if a host has no DNS records, it's often a "stale" or internal-only asset.

### Step 5: Deep HTTP Probing (`httpx`)
The final step is a deep dive into the web services running on these hosts.
- Captures status codes and response times.
- **TLS Analysis**: Captures certificate issuer and expiration date.
- **Redirect Tracking**: Records the full path from `http` to `https` or external targets.
- **Fingerprinting**: Identifies the technology stack (e.g., React, WordPress, Cloudflare).
- **Metadata**: Grabs page titles and `Server` headers.

---

## Batch Reconnaissance

To run the entire pipeline at once, use the `batch` command:

```bash
balebos batch domains.txt
```

### What happens during batch:
1.  **Loader**: Loads all domains from the text file.
2.  **Step 1/5**: Scans each domain and adds it to inventory.
3.  **Step 2/5**: Runs `rdap`/`whois` for each root domain.
4.  **Step 3/5**: Runs `subfinder` for each domain.
5.  **Step 4/5**: Collects all unique hosts (root + subdomains) and runs `dnsx` **in high-concurrency batch mode**.
6.  **Step 5/5**: Runs `httpx` on all hosts **in a single streaming process**.
7.  **Summary**: Displays a comprehensive report of assets found and checks completed.

---

## Technical Invariants

- **Performance**: Balebos is optimized for speed. By streaming data directly to tool pipelines, it can process hundreds of probes per second without the overhead of restarting processes.
- **Persistence**: Every step writes its results immediately to `data/inventory.jsonl`.
 Even if the batch process is interrupted, the data already collected is safe.
- **Non-Destructive**: Balebos never deletes old observations. It builds a timeline of facts.
- **Tool Dependency**: Ensure `subfinder`, `rdap`, `dnsx`, and `httpx` are in your `$PATH`. Balebos will provide a clear error message if a tool is missing.
