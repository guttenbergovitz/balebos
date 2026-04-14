# Organisation Model Guide

Balebos's organisational model lets you represent your company structure and track ownership of assets. Rather than treating domains as orphaned items, you can link them to projects, companies, and people—providing essential context for risk assessment and incident response.

## Overview

The organisational model is a hierarchy:

```
Brand (umbrella company)
  └── Company (legal entity or contractor)
        └── Project (cost centre / team)
              ├── Asset (domain/subdomain assigned to project)
              └── People / Companies (with roles: Owner, Manager, Technician)
```

This structure answers critical operational questions:
- Who owns this asset?
- Which team is responsible for responding to outages?
- Which cost centre should be billed for this infrastructure?
- Why does this domain exist?

## Core Concepts

### Brands

A brand groups related companies under a single umbrella. Typically your company name or holding company.

**Use brands for:**
- Multi-subsidiary organisations
- Company rebranding (keep old brand for legacy assets)
- Separating internal and external operations
- Cost allocation at the corporate level

**Example:**
- Acme Corporation (brand)
  - Acme Deutschland GmbH (internal company)
  - Acme France SARL (internal company)
  - Acme Consulting (contractor brand, external)

### Companies

A company is a legal entity—either internal (owned by a brand) or external (contractor/vendor).

**Internal companies:**
- Belong to a brand
- Use for subsidiaries, divisions, or distinct business units
- Track as part of your organisation structure

**External companies:**
- Have no brand (null)
- Used for contractors, vendors, service providers
- Can be assigned to projects but don't "own" them

### Projects

A project is a cost centre or team within a company. It's where work actually happens.

**Use projects for:**
- Cost centre tracking (MPK / Kostenstelle)
- Team structure (Platform Team, Product Team, DevOps)
- Service owners (all assets in a project are the project's responsibility)
- Budget allocation

**Example:**
```
Acme Deutschland GmbH
  └── Project: Website Platform (cost centre: DE-2024-001)
        └── api.example.de
        └── www.example.de
        └── cdn.example.de
```

### People

Individuals who can own or manage projects. Track name, email, and optional contact details.

**Used for:**
- Escalation contacts
- On-call lists
- Incident response
- Compliance records

### Role Assignments

Links a person or company to a project with a specific responsibility.

**Person roles:**
- **Owner** - Ultimate responsibility, sets strategy
- **Manager** - Day-to-day operations
- **Technician** - Hands-on implementation and troubleshooting

**Company roles (contractor assignment):**
- **Development** - Code and architecture
- **DevOps** - Infrastructure and deployment
- **Design** - UI/UX and branding
- **Consultancy** - Advisory and planning
- **Security** - Assessments and hardening

## CLI Commands

### Brand Management

Create a brand:
```bash
balebos org brand add "Acme Corporation"
```

List all brands:
```bash
balebos org brand list
```

Output:
```
Brands
═════════════════════════════════════════

1234567890   Acme Corporation
2345678901   Acme Legacy (pre-rebranding)
```

### Company Management

Create an internal company (with brand):
```bash
balebos org company add "Acme Deutschland GmbH" DEU --brand 1234567890
```

Create an external company (contractor):
```bash
balebos org company add "External Agency" POL
```

List companies:
```bash
balebos org company list
```

Output:
```
Companies
═════════════════════════════════════════

ID              Name                        Country   Brand
1111111111      Acme Deutschland GmbH       DEU       1234567890
2222222222      Acme France SARL            FRA       1234567890
3333333333      External Agency             POL       (none)
```

### Person Management

Add a person:
```bash
balebos org person add "Jan" "Kowalski" "jan@example.com" --phone "+48123456789"
```

The phone number is optional. Email is required (for contact purposes).

List people:
```bash
balebos org person list
```

Output:
```
People
═════════════════════════════════════════

ID              First Name  Last Name    Email
1111111111      Jan         Kowalski     jan@example.com
2222222222      Maria       Schmidt      maria@example.com
3333333333      Jean        Dupont       jean@example.com
```

### Project Management

Create a project (belongs to a company):
```bash
balebos org project add "Website Platform" --company 1111111111
```

List projects:
```bash
balebos org project list
```

Output:
```
Projects
═════════════════════════════════════════

ID              Name                    Company
3333333333      Website Platform        Acme Deutschland GmbH
4444444444      API Services            Acme Deutschland GmbH
5555555555      DevOps Platform         Acme Deutschland GmbH
```

### Asset Assignment

Assign an asset (domain) to a project:
```bash
balebos org assign asset api.example.com --project 3333333333
```

List project assets:
```bash
balebos org project show 3333333333
```

Output:
```
Project: Website Platform
Company: Acme Deutschland GmbH
Brand: Acme Corporation

Assigned Assets (7):
  • api.example.com
  • www.example.com
  • cdn.example.com
  • mail.example.com
  • ...

Team Members:
  Owner:      Jan Kowalski (jan@example.com)
  Manager:    Maria Schmidt (maria@example.com)
  Tech Lead:  Jean Dupont (jean@example.com)
```

### Role Assignment (People)

Assign a person to a project with a role:
```bash
balebos org assign person 1111111111 --project 3333333333 --role owner
balebos org assign person 2222222222 --project 3333333333 --role manager
balebos org assign person 3333333333 --project 3333333333 --role technician
```

### Role Assignment (Contractors)

Assign an external company to a project with a contractor role:
```bash
balebos org assign contractor 3333333333 --project 3333333333 --role development
balebos org assign contractor 3333333333 --project 3333333333 --role devops
```

## TUI Integration

The Organisation tab in the interactive dashboard provides:

1. **Browse structure** - Navigate brands → companies → projects
2. **View team** - See who's assigned to each project
3. **Manage assignments** - Press `a` to assign assets or people
4. **See assets** - View all domains belonging to a project
5. **Edit contacts** - Update person information

## Real-World Examples

### Example 1: Multi-Subsidiary Company

You own multiple companies under one brand. Each company has its own projects and teams.

```bash
# Create brand
balebos org brand add "Acme Group"

# Create subsidiaries
balebos org company add "Acme Deutschland" DEU --brand <brand-id>
balebos org company add "Acme France" FRA --brand <brand-id>
balebos org company add "Acme UK" GBR --brand <brand-id>

# Create projects in each
balebos org project add "Platform" --company <de-company-id>
balebos org project add "Platform" --company <fr-company-id>

# Assign assets
balebos org assign asset api.de.example.com --project <de-platform-id>
balebos org assign asset api.fr.example.com --project <fr-platform-id>

# Assign team members
balebos org assign person <jan-id> --project <de-platform-id> --role owner
balebos org assign person <maria-id> --project <fr-platform-id> --role owner
```

Now when you investigate `api.de.example.com`, you immediately know it's Acme Deutschland's Platform team, and Jan is the owner.

### Example 2: Mixed Internal and Contractor Teams

You have internal teams and contractor partners working on the same project.

```bash
# Create company
balebos org company add "Acme Tech" DEU --brand <brand-id>

# Create project
balebos org project add "Microservices Platform" --company <company-id>

# Assign internal team
balebos org add person "Alice" "Engineer" "alice@acme.com"
balebos org assign person <alice-id> --project <project-id> --role owner

# Assign external contractor
balebos org company add "TechConsult GmbH" DEU  # external, no brand
balebos org assign contractor <contractor-id> --project <project-id> --role development
```

### Example 3: Legacy Brand with Decommissioning

Old brand still has live infrastructure but you're consolidating.

```bash
# Create legacy brand (for historical tracking)
balebos org brand add "Acme Legacy"

# Create legacy company
balebos org company add "Acme Old Products" DEU --brand <legacy-brand-id>

# Create sunset project
balebos org project add "Deprecation Migration" --company <company-id>

# Assign legacy assets
balebos org assign asset oldapi.example.com --project <sunset-project-id>
balebos org assign asset legacy.example.com --project <sunset-project-id>

# Track progress in risk report
balebos risk report --by-brand  # See how much risk is in legacy brand
```

## Data Storage

Organisational data is stored as append-only records in Parquet:

**Brand record:**
```json
{
  "item_type": "org_brand",
  "id": 1234567890,
  "name": "Acme Corporation",
  "created_at": "2026-03-21T10:00:00Z"
}
```

**Company record:**
```json
{
  "item_type": "org_company",
  "id": 1111111111,
  "name": "Acme Deutschland GmbH",
  "country": "DEU",
  "brand": 1234567890,
  "created_at": "2026-03-21T10:00:00Z"
}
```

**Person record:**
```json
{
  "item_type": "org_person",
  "id": 9876543210,
  "first_name": "Jan",
  "last_name": "Kowalski",
  "email": "jan@example.com",
  "phone": null,
  "created_at": "2026-03-21T10:00:00Z"
}
```

**Project record:**
```json
{
  "item_type": "org_project",
  "id": 3333333333,
  "name": "Website Platform",
  "company": 1111111111,
  "created_at": "2026-03-21T10:00:00Z"
}
```

**Asset assignment:**
```json
{
  "item_type": "org_asset_assignment",
  "host": "api.example.com",
  "project": 3333333333,
  "created_at": "2026-03-21T10:00:00Z"
}
```

**Role assignment:**
```json
{
  "item_type": "org_role_assignment",
  "project": 3333333333,
  "role": "Owner",
  "person": 9876543210,
  "company": null,
  "created_at": "2026-03-21T10:00:00Z"
}
```

See [Data Model](../specs/data-model.md) for complete schema documentation.

## Integration with Other Features

### Risk Scoring

Assets without project ownership generate governance findings, increasing risk scores. Assigning assets to projects immediately lowers risk:

```bash
balebos risk show orphaned-asset.example.com  # Shows ownership finding
balebos org assign asset orphaned-asset.example.com --project <project-id>
balebos risk show orphaned-asset.example.com  # Governance finding resolved
```

### Findings

Findings now show which team should respond. When you receive a critical finding:

```bash
balebos finding show <id>
# Shows: Assigned to Website Platform project
# Owner: Jan Kowalski (jan@example.com)
# Notify: jan@example.com
```

### Lifecycle Management

Asset lifecycle state should align with project state:

```bash
# Active project → Active assets
balebos asset lifecycle set api.example.com active
balebos org assign asset api.example.com --project <active-project-id>

# Deprecated project → Deprecated assets
balebos asset lifecycle set oldapi.example.com deprecated
balebos org assign asset oldapi.example.com --project <sunset-project-id>
```

## Common Workflows

### Onboarding a New Team

```bash
# Create team structure
balebos org company add "Platform Team" DEU --brand <brand-id>
balebos org project add "Kubernetes Cluster" --company <company-id>

# Add team members
balebos org person add "Alice" "Lead" "alice@example.com"
balebos org person add "Bob" "Engineer" "bob@example.com"
balebos org person add "Carol" "Engineer" "carol@example.com"

# Assign roles
balebos org assign person <alice-id> --project <project-id> --role owner
balebos org assign person <bob-id> --project <project-id> --role manager
balebos org assign person <carol-id> --project <project-id> --role technician

# Assign their infrastructure
balebos org assign asset k8s.example.com --project <project-id>
balebos org assign asset ingress.example.com --project <project-id>
```

### Incident Response

When an incident happens, the organisation structure helps you find the right team:

```bash
balebos finding show critical-issue-id
# Shows: Project: Platform Team → Kubernetes Cluster
#        Owner: Alice Lead (alice@example.com)
#        Manager: Bob (bob@example.com)

# Send alert to right people
echo "Critical issue in your project" | mail alice@example.com, bob@example.com
```

### Audit Trail

Show who owns what for compliance:

```bash
balebos org project list
balebos org project show <project-id>  # See all people and contractors assigned
```

## Troubleshooting

### "Asset not in any project"

This is normal. Not all assets need project assignment. Use risk reports to find unassigned assets:

```bash
balebos risk report --unassigned
```

Then assign them:

```bash
balebos org assign asset orphaned.example.com --project <project-id>
```

### "Invalid country code"

Country codes must be ISO 3166-1 alpha-3 (3-letter codes):
- DEU = Germany
- FRA = France
- GBR = United Kingdom
- POL = Poland
- USA = United States

See [ISO 3166-1](https://en.wikipedia.org/wiki/ISO_3166-1_alpha-3) for complete list.

### "Person already assigned"

You can't assign the same person to the same project twice. To change their role:

```bash
# Remove (note: future feature)
balebos org unassign person <id> --project <project-id>

# Re-assign with new role
balebos org assign person <id> --project <project-id> --role manager
```

## See Also

- [Asset Lifecycle](asset-lifecycle.md) - Aligning lifecycle state with organisational context
- [Risk Scoring](risk-scoring.md) - How organisation structure feeds into risk assessment
- [Findings Management](findings.md) - Notifications and escalation
- [Data Model](../specs/data-model.md) - Complete schema for organisational records
