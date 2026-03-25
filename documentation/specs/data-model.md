# Data Model Specification

This document defines the storage format and data structures used by Balebos. These structures apply to both the newline-delimited JSON (JSONL) files and the `data` column in the DuckDB backend.

## Core Principles

1.  **Heterogeneous Log**: Multiple record types (domains, observations, results) coexist in the same storage (file or table).
2.  **Append-Only**: Records are never updated or deleted. New observations are simply appended. In DuckDB, each record is an event in the `events` table.
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
- **`mx`**: Array of Mail Exchange servers
- **`ns`**: Array of Name Servers
- **`txt`**: Array of TXT records (verification, SPF, etc.)
- **`soa`**: Array of SOA record strings
- **`checked_at`**: RFC3339 Timestamp

**Example:**
```json
{
  "item_type": "dns_record",
  "target": "example.com",
  "a": ["93.184.216.34"],
  "aaaa": [],
  "mx": ["mail.example.com"],
  "ns": ["ns1.example.com"],
  "txt": ["v=spf1 include:_spf.example.com ~all"],
  "soa": ["ns:ns1.example.com mailbox:admin.example.com"],
  "checked_at": "2026-03-21T10:05:00Z"
}
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
- **`tls`**: Optional `TlsObservation` object
- **`error_kind`**: Network error category (if any)

### TlsObservation Object
- **`subject_dn`**: Distinguished Name of the subject
- **`issuer_dn`**: Distinguished Name of the issuer
- **`not_before`**: Validity start timestamp
- **`not_after`**: Validity end timestamp
- **`fingerprint_sha256`**: SHA256 certificate fingerprint

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
  "tls": {
    "subject_dn": "CN=example.com",
    "issuer_dn": "CN=Let's Encrypt Authority X3",
    "not_before": "2026-01-01T00:00:00Z",
    "not_after": "2026-04-01T00:00:00Z",
    "fingerprint_sha256": "..."
  },
  "tech": ["HSTS", "Varnish"],
  "title": "Example Domain",
  "checked_at": "2026-03-21T10:10:00Z"
}
```

---

## 5. Organizational Records

Balebos supports organizational context for assets, allowing you to track ownership, responsibility, and cost allocation.

### Hierarchy

```
Brand (e.g., "Acme Corp")
  └── Company (e.g., "Acme Deutschland GmbH", brand=Some)
        └── Project (cost center / MPK)
              └── Asset (host assignment)

External Contractor (brand=None)
  └── Assigned to Project via RoleAssignment
```

### 5.1 Person Record

- **`item_type`**: `"org_person"`
- **`id`**: Unique numeric ID (timestamp-based)
- **`first_name`**: First name
- **`last_name`**: Last name
- **`email`**: Email address
- **`phone`**: Optional phone number
- **`created_at`**: RFC3339 Timestamp

**Example:**
```json
{"item_type":"org_person","id":1234567890,"first_name":"Jan","last_name":"Kowalski","email":"jan@example.com","phone":null,"created_at":"2026-03-21T10:00:00Z"}
```

### 5.2 Brand Record

- **`item_type`**: `"org_brand"`
- **`id`**: Unique numeric ID
- **`name`**: Brand name
- **`created_at`**: RFC3339 Timestamp

**Example:**
```json
{"item_type":"org_brand","id":1234567890,"name":"Acme Corp","created_at":"2026-03-21T10:00:00Z"}
```

### 5.3 Company Record

Companies are legal entities. Internal companies belong to a Brand; external contractors have no brand.

- **`item_type`**: `"org_company"`
- **`id`**: Unique numeric ID
- **`name`**: Company name
- **`country`**: ISO 3166-1 alpha-3 country code
- **`brand`**: Brand ID (null for external contractors)
- **`created_at`**: RFC3339 Timestamp

**Example (internal company):**
```json
{"item_type":"org_company","id":1234567890,"name":"Acme Deutschland GmbH","country":"DEU","brand":9876543210,"created_at":"2026-03-21T10:00:00Z"}
```

**Example (external contractor):**
```json
{"item_type":"org_company","id":1234567891,"name":"External Agency","country":"POL","brand":null,"created_at":"2026-03-21T10:00:00Z"}
```

### 5.4 Project Record

Projects are cost centers (MPK / Kostenstelle) belonging to a Company. Brand is derived from the Company.

- **`item_type`**: `"org_project"`
- **`id`**: Unique numeric ID
- **`name`**: Project name
- **`company`**: Company ID
- **`created_at`**: RFC3339 Timestamp

**Example:**
```json
{"item_type":"org_project","id":1234567890,"name":"Website Relaunch","company":9876543210,"created_at":"2026-03-21T10:00:00Z"}
```

### 5.5 Asset Assignment Record

Links a host (domain/subdomain) to a Project.

- **`item_type`**: `"org_asset_assignment"`
- **`host`**: Domain or subdomain name
- **`project`**: Project ID
- **`created_at`**: RFC3339 Timestamp

**Example:**
```json
{"item_type":"org_asset_assignment","host":"api.example.com","project":1234567890,"created_at":"2026-03-21T10:00:00Z"}
```

### 5.6 Role Assignment Record

Assigns a Person or Company to a Project with a specific role (Owner, Manager, Technician).

- **`item_type`**: `"org_role_assignment"`
- **`project`**: Project ID
- **`role`**: One of `"Owner"`, `"Manager"`, `"Technician"`
- **`person`**: Person ID (null if company assignment)
- **`company`**: Company ID (null if person assignment)
- **`created_at`**: RFC3339 Timestamp

**Example (person assignment):**
```json
{"item_type":"org_role_assignment","project":1234567890,"role":"Owner","person":111222333,"company":null,"created_at":"2026-03-21T10:00:00Z"}
```

**Example (company assignment):**
```json
{"item_type":"org_role_assignment","project":1234567890,"role":"Technician","person":null,"company":444555666,"created_at":"2026-03-21T10:00:00Z"}
```

### 5.7 External Contractor Assignment Record

Assigns an external company (no brand) to a Project with a specific contractor role.

- **`item_type`**: `"org_contractor_assignment"`
- **`project`**: Project ID
- **`company`**: Company ID (must be external, i.e., `brand: null`)
- **`role`**: One of `"Development"`, `"DevOps"`, `"Design"`, `"Consultancy"`, `"Security"`
- **`created_at`**: RFC3339 Timestamp

**Example:**
```json
{"item_type":"org_contractor_assignment","project":1234567890,"company":444555666,"role":"Development","created_at":"2026-03-21T10:00:00Z"}
```

---

## 6. Domain Registration Record

Captures RDAP/WHOIS registration data for a domain, including registrar, expiration dates, and contact information.

- **`item_type`**: `"org_domain_registration"`
- **`domain`**: Domain name
- **`registrar`**: Registrar name (optional)
- **`registered_at`**: Domain registration date (optional)
- **`expires_at`**: Domain expiration date (optional)
- **`updated_at`**: Last update date (optional)
- **`status`**: Array of EPP status codes (e.g., `"clientTransferProhibited"`)
- **`registrant`**: Optional `DomainContact` object
- **`admin`**: Optional `DomainContact` object
- **`tech`**: Optional `DomainContact` object
- **`source`**: Data source (`"rdap"` or `"whois"`)
- **`created_at`**: RFC3339 Timestamp

### DomainContact Object

- **`name`**: Contact name (optional)
- **`organization`**: Organization name (optional)
- **`email`**: Contact email (optional)

**Example:**
```json
{
  "item_type": "org_domain_registration",
  "domain": "example.com",
  "registrar": "Example Registrar Inc.",
  "registered_at": "2020-01-15T00:00:00Z",
  "expires_at": "2027-01-15T00:00:00Z",
  "updated_at": "2026-01-10T12:00:00Z",
  "status": ["clientTransferProhibited", "clientDeleteProhibited"],
  "registrant": {
    "name": "John Doe",
    "organization": "Example Corp",
    "email": "admin@example.com"
  },
  "admin": null,
  "tech": null,
  "source": "rdap",
  "created_at": "2026-03-21T10:15:00Z"
}
```
