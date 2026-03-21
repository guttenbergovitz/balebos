# Data Model Specification

This document defines the storage format and data structures used by Balebos in the append-only JSONL inventory.

## Core Principles

1.  **Heterogeneous Log**: Multiple record types (domains, observations, results) coexist in the same `.jsonl` file.
2.  **Append-Only**: Records are never updated or deleted. New observations are simply appended.
3.  **Record Header**: Every record must contain an `item_type` field to allow for correct parsing.

---

## 1. Domain Record

Represents a root asset added to the inventory.

- **`item_type`**: `"domain"`
- **`id`**: UUID v4
- **`name`**: Normalized domain name (lowercase, no protocol)
- **`discovered_at`**: RFC3339 Timestamp

**Example:**
```json
{"item_type":"domain","id":"8a594a53-c36c-49f2-9376-34a520e4fa18","name":"example.com","discovered_at":"2026-03-21T10:00:00Z"}
```

---

## 2. Subdomain Record

Represents an asset discovered via automated probing (e.g., `subfinder`).

- **`item_type`**: `"subdomain"`
- **`id`**: Unique string (usually `sub:name`)
- **`name`**: Full subdomain name
- **`source`**: Tool that discovered the asset (e.g., `"subfinder"`)
- **`original_input`**: The root domain used for discovery

**Example:**
```json
{"item_type":"subdomain","id":"sub:api.example.com","name":"api.example.com","source":"subfinder","original_input":"example.com"}
```

---

## 3. DNS Observation

Captured facts about network-level resolution via `dnsx`.

- **`item_type`**: `"dns_record"`
- **`target`**: Hostname being resolved
- **`a`**: Array of IPv4 addresses
- **`aaaa`**: Array of IPv6 addresses
- **`cname`**: Array of CNAME aliases
- **`checked_at`**: RFC3339 Timestamp

**Example:**
```json
{"item_type":"dns_record","target":"example.com","a":["93.184.216.34"],"aaaa":[],"cname":[],"checked_at":"2026-03-21T10:05:00Z"}
```

---

## 4. HTTP Observation

Deep probe results including tech stack and redirect history via `httpx`.

- **`item_type`**: `"http_check"`
- **`target`**: Target host
- **`status`**: Final HTTP status code (optional on error)
- **`response_time_ms`**: Total duration of the request
- **`final_url`**: The destination URL after all redirects
- **`redirect_chain`**: Array of `RedirectHop` objects
- **`tech`**: Array of detected technologies (e.g., `"Nginx"`, `"HSTS"`)
- **`title`**: HTML `<title>` of the page
- **`webserver`**: `Server` header value
- **`error_kind`**: Network error category (if any)

### RedirectHop Object
- **`url`**: Source URL of the hop
- **`status`**: Status code (301, 302, etc.)
- **`redirect_to`**: Destination URL of the hop

**Example:**
```json
{
  "item_type": "http_check",
  "target": "example.com",
  "status": 200,
  "response_time_ms": 120,
  "final_url": "https://example.com/",
  "redirect_chain": [
    {"url": "http://example.com", "status": 301, "redirect_to": "https://example.com/"}
  ],
  "tech": ["HSTS", "Varnish"],
  "title": "Example Domain",
  "checked_at": "2026-03-21T10:10:00Z"
}
```
