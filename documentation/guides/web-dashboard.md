# Web Dashboard

Balebos can export an interactive static web dashboard for sharing infrastructure status with business stakeholders, managers, or anyone who doesn't use the terminal.

The dashboard is a fully static site — no server-side processing required. It runs entirely in the browser, fetching JSON data files generated from your Parquet storage.

## What It Looks Like

- **Solarized Dark** theme with **JetBrains Mono** for technical data
- **Foldable TLD groups** — root domains always visible, subdomains collapsed by default (click ▶ to expand)
- **Side panel detail view** — click any asset to slide in a panel with full details
- **Technology pills** — all detected technologies shown as inline coloured tags
- **Client-side pagination** — 15 domains per page with prev/next controls
- **Search and filter** — filter by name, lifecycle state, or risk level
- **Sortable columns** — click headers to sort by name or risk score
- **Three tabs**: Inventory, Findings, Risk Distribution

## Prerequisites

- [Zola](https://www.getzola.org/documentation/getting-started/installation/) — static site generator
- Balebos built from source (for the `web export` command)

## Building the Dashboard

### Quick Build (Recommended)

```bash
./build-web.sh
```

This script:
1. Runs `cargo run --release -- web export --output-dir web/static/data` to generate JSON data files
2. Runs `zola build` to compile the static site
3. Outputs to `web/public/`

### Manual Build

```bash
# Export data from Parquet storage
cargo run --release -- web export --output-dir web/static/data

# Build the site
cd web && zola build

# Preview locally
cd web && zola serve
```

## Deploying

The `web/public/` directory contains a fully static site. Copy it to any web server:

```bash
# nginx
cp -r web/public/* /var/www/html/

# GitHub Pages
# Push web/public contents to gh-pages branch

# S3 + CloudFront
aws s3 sync web/public/ s3://your-bucket/

# Netlify
cd web/public && netlify deploy --prod
```

## Data Files

The export generates three JSON files in `web/static/data/`:

| File | Content |
|------|---------|
| `assets.json` | All domains and subdomains with DNS, HTTP, TLS, hosting, and tech stack |
| `findings.json` | All findings with severity, status, and target |
| `metadata.json` | Summary: counts, timestamps, critical/high risk tallies |

These files are **gitignored** — they are recreated on each export. Only commit the templates, CSS, and config.

## Customising

### Changing the Theme

Edit `web/static/css/main.css`:

```css
:root {
  --base03: #002b36;  /* background */
  --base02: #073642;  /* panels */
  --accent: #2aa198;  /* cyan highlights */
  /* ... */
}
```

### Changing Items Per Page

Edit `web/templates/base.html` in the `assetTable` Alpine.js component:

```javascript
Alpine.data('assetTable', () => ({
    // ...
    perPage: 15,  // change this
    // ...
}));
```

### Adding New Detail Sections

The side panel is in `web/templates/partials/asset-table.html`. Add new `.panel-section` blocks inside the `.panel-body`.

## Architecture

```
web/
├── config.toml              # Zola site config
├── content/
│   └── _index.md            # Homepage (points to index.html template)
├── static/
│   ├── css/main.css         # Solarized Dark + JetBrains Mono styles
│   └── data/                # Generated JSON (gitignored)
│       ├── assets.json
│       ├── findings.json
│       └── metadata.json
└── templates/
    ├── base.html            # Layout + Alpine.js component definitions
    ├── index.html           # Tab navigation (Inventory/Findings/Risk)
    └── partials/
        ├── asset-table.html # Foldable groups + pagination + side panel
        └── findings-list.html
```

## Troubleshooting

### "0 pages (0 orphan)" from Zola

This is normal. Zola counts content pages (in `content/`), but our homepage is a section (`_index.md`) with a custom template. The site builds correctly — check `web/public/index.html` exists.

### Assets not showing in the table

Check the browser console for fetch errors. The JSON files must be served from `/data/` (Zola copies `static/data/` to `public/data/`). If using a custom server, ensure the `/data/` path is accessible.

### Zola not found

Install Zola:

```bash
# macOS
brew install zola

# Linux
# Download from https://github.com/getzola/zola/releases

# Verify
zola --version
```

## Security Considerations

The dashboard is a **read-only static site**. It contains no authentication, no write endpoints, and no server-side code. Anyone with access to the deployed site can see all exported data.

- Do not deploy to public internet without considering data sensitivity
- Use VPN, basic auth at the reverse proxy, or IP whitelisting for access control
- The exported JSON contains full DNS records, HTTP responses, and TLS details
