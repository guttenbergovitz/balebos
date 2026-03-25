# Tag Management

This guide explains how to use tags to organize and filter assets in your inventory.

## Overview

Tags are labels that can be assigned to assets (domains and subdomains). They help organize inventory for review workflows, prioritization, or categorization.

Tags are stored as append-only records in JSONL, following the same immutability principle as other Balebos data.

## Command Reference

### Adding Tags

```bash
balebos tag add <asset_name> <tag>
```

Assigns a tag to an asset. Multiple tags can be assigned to the same asset.

**Examples:**
```bash
balebos tag add example.com to-review
balebos tag add example.com priority-high
balebos tag add shop.example.com production
```

### Removing Tags

```bash
balebos tag remove <asset_name> <tag>
```

Marks a tag as removed. The original tag record remains in storage (append-only), but it will be filtered out in queries.

**Example:**
```bash
balebos tag remove example.com to-review
```

### Listing Tags

**All tags:**
```bash
balebos tag list
```

Shows all assets with their assigned tags.

**Tags for a specific asset:**
```bash
balebos tag list <asset_name>
```

**Example:**
```bash
balebos tag list example.com
```

Output:
```
Tags for 'example.com':
  • priority-high
  • to-review
```

### List Unique Tag Names

```bash
balebos tag list-tags
```

Shows all unique tag names currently in use.

### Find Assets by Tag

```bash
balebos tag by-tag <tag>
```

Lists all assets that have a specific tag.

**Example:**
```bash
balebos tag by-tag priority-high
```

Output:
```
Assets tagged with 'priority-high':
  • example.com

Total: 1 asset(s)
```

### Bulk Tagging

In the TUI, you can tag multiple assets at once:

1. Select multiple assets using `Space` key
2. Press `T` (Shift+t) to open tag selector
3. Select or create tag
4. All selected assets are tagged with the same tag

Similarly, remove tags from multiple selected assets:

1. Select multiple assets using `Space` key
2. Press `d` to open remove tag modal
3. Select tag to remove from all selected assets

## Export with Tag Filtering

Export assets with optional tag filtering:

```bash
balebos status --export csv --filter-tag priority-high
```

Only assets with the specified tag will be exported. This is useful for:
- Exporting domains to review before making changes
- Creating reports for specific asset categories
- Sharing prioritized asset lists with team members

## Common Workflows

### Review Workflow
Flag domains for review and track progress:

```bash
# Mark domains
balebos tag add example.com to-review
balebos tag add test.com to-review

# List all domains to review
balebos tag by-tag to-review

# After review, remove the tag
balebos tag remove example.com to-review
```

### Bulk Review Processing (TUI)
Mark multiple domains for review and process them:

1. In TUI, navigate to inventory
2. Select multiple domains with `Space`
3. Press `T` (Shift+t) and select/create `to-review` tag
4. All selected domains are tagged
5. Later, select same domains and press `d` to bulk remove `to-review` tag

### Prioritization
Organize assets by priority:

```bash
balebos tag add critical-app.com priority-high
balebos tag add backup-site.com priority-low

# Filter by priority
balebos tag by-tag priority-high
```

### Environment Categorization
Separate production from staging environments:

```bash
balebos tag add prod-api.example.com production
balebos tag add staging-api.example.com staging

# See all production domains
balebos tag by-tag production
```

## Storage Format

Tags are stored as records in JSONL with the following structure:

```json
{
  "item_type": "asset_tag",
  "id": "uuid",
  "asset_name": "example.com",
  "tag": "to-review",
  "tagged_at": "2026-03-25T16:32:45.707236Z",
  "tagged_by": "cli"
}
```

## Tag Naming Conventions

Recommended naming patterns:
- Use kebab-case: `priority-high`, `needs-review`
- Be consistent across your inventory
- Consider prefixes for categories:
  - `priority-*`: priority-high, priority-low
  - `env-*`: env-production, env-staging
  - `status-*`: status-active, status-deprecated

## TUI Integration

The tagging system is fully integrated with the TUI (terminal user interface).

### Keyboard Shortcuts

In the Inventory tab:

- **`t`** - Open tag filter selector (choose tag to filter by)
- **`T` (Shift+t)** - Add tag to selected asset (opens tag selector)
- **`d`** - Remove tag from selected asset (opens modal)

**In Tag Filter Selector (t):**
- **`↑`/`↓` or `j`/`k`** - Navigate through available tags
- **`Enter`** - Apply selected tag filter
- **`c` or `x`** - Clear filter (show all assets)
- **`Esc`** - Cancel without changing filter

**In Tag Selector for Adding (T):**
- **`↑`/`↓` or `j`/`k`** - Navigate through existing tags
- **`Enter`** - Select highlighted tag
- **`n`** - Create new tag (manual entry)
- **`Esc`** - Cancel

### Visual Feedback

- Title bar shows active filter: `Inventory [Tag: priority-high]`
- Filter bar shows active tag filter: `TAG: priority-high` (green badge)
- Tag selector shows all available tags with highlight
- Add Tag modal displays current and available tags
- Remove Tag modal shows current tags list
- Status bar (bottom) shows notifications and idle quotes when not processing

### Status Bar

The status bar at the bottom displays:
- **Real-time notifications** from tag operations and scans
- **Idle quotes** from famous programmers and Monty Python (rotate every 30 seconds)
- **Validation errors** and operation results
- Messages auto-hide after completion to keep interface clean

### Workflow Example

**Adding an existing tag:**
1. Navigate to asset with `j`/`k` keys
2. Press `T` (Shift+t) to open tag selector
3. Use `↑`/`↓` or `j`/`k` to navigate tags
4. Press `Enter` to select highlighted tag
5. Asset is refreshed automatically

**Adding a new tag:**
1. Navigate to asset with `j`/`k` keys  
2. Press `T` (Shift+t) to open tag selector
3. Press `n` to switch to manual entry
4. Type new tag name (e.g., "priority-high")
5. Press `Enter` to confirm (or `Esc` to cancel)
6. Asset is refreshed automatically

**Filtering by tag:**
1. Press `t` to open tag filter selector
2. Use `↑`/`↓` or `j`/`k` to navigate available tags
3. Press `Enter` to apply filter
4. Only assets with selected tag are displayed
5. Active filter shown in title bar AND filter bar
6. Press `t` again and `c`/`x` to clear filter, or `Esc` to keep current

**Removing a tag:**
1. Navigate to asset
2. Press `d`
3. See list of current tags
4. Type tag name to remove
5. Press `Enter` to confirm

Tag data is loaded into the TUI model at startup and available in `app.model.asset_tags` as a `BTreeMap<String, BTreeSet<String>>`.

The inventory state includes a `filter_tag` field for filtering the asset list by tag.

