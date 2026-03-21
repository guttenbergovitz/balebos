/// Tree visualization module for displaying domain hierarchies.
///
/// This module provides functionality to display domains and their discovered subdomains
/// in a hierarchical tree format. It helps visualize the organizational structure of your
/// infrastructure by showing which domains have nested subdomains.
///
/// # Features
///
/// - Displays root domains (marked with 🔵) at the top level
/// - Shows subdomains (marked with ◇) nested under their parent domains
/// - Supports multi-level nesting (subdomains of subdomains)
/// - Preserves parent-child relationships from subdomain discovery
///
/// # Example
///
/// For a domain structure like:
/// - `example.com` (root domain)
///   - `api.example.com` (subdomain)
///     - `dev.api.example.com` (nested subdomain)
///
/// The output would be:
/// ```text
/// ├── 🔵 example.com
/// │   └── ◇ api
/// │       └── ◇ dev
/// ```
use crate::storage::{read_records, JsonlStorage};
use std::collections::BTreeMap;
use std::io;

// Tree node representing a domain or subdomain with its children
#[derive(Debug, Clone)]
struct TreeNode {
    name: String,
    item_type: String, // "domain" or "subdomain"
    children: BTreeMap<String, TreeNode>,
}

impl TreeNode {
    fn new(name: String, item_type: String) -> Self {
        TreeNode {
            name,
            item_type,
            children: BTreeMap::new(),
        }
    }
}

/// Build a hierarchical tree of domains and subdomains
/// Returns root nodes (base domains) with their subdomain hierarchy
fn build_domain_tree(storage: &JsonlStorage) -> Result<Vec<TreeNode>, io::Error> {
    let items = read_records(storage)?.items;

    let domain_records: Vec<String> = items
        .iter()
        .filter(|item| item.item_type == "domain")
        .map(|item| item.name.clone())
        .collect();

    let mut all_items: Vec<(String, String)> = items
        .into_iter()
        .map(|item| (item.name, item.item_type))
        .collect();

    all_items.sort_by(|a, b| a.0.cmp(&b.0));
    all_items.dedup_by(|a, b| a.0 == b.0);

    // Build tree structure: only add items that are either root domains or subdomains of existing domains
    let mut roots: BTreeMap<String, TreeNode> = BTreeMap::new();

    for (fqdn, item_type) in all_items {
        // Check if this is a root domain record
        if item_type == "domain" {
            roots.insert(fqdn.clone(), TreeNode::new(fqdn, "domain".to_string()));
        } else if item_type == "subdomain" {
            // Find which domain this subdomain belongs to
            // A subdomain belongs to a domain if the domain matches its suffix
            if let Some(parent_domain) = find_parent_domain(&fqdn, &domain_records) {
                if let Some(root) = roots.get_mut(&parent_domain) {
                    insert_subdomain_into_tree(root, &parent_domain, &fqdn);
                }
            }
        }
    }

    Ok(roots.into_values().collect())
}

/// Find which domain a subdomain belongs to
fn find_parent_domain(fqdn: &str, domains: &[String]) -> Option<String> {
    // Look for exact match or suffix match
    for domain in domains {
        if fqdn == domain {
            return Some(domain.clone());
        }
        if fqdn.ends_with(&format!(".{}", domain)) {
            return Some(domain.clone());
        }
    }
    None
}

/// Recursively insert a subdomain into the tree
fn insert_subdomain_into_tree(node: &mut TreeNode, parent_domain: &str, fqdn: &str) {
    // Get the subdomain part (everything before the parent domain)
    if let Some(subdomain_part) = fqdn.strip_suffix(&format!(".{}", parent_domain)) {
        insert_into_node(node, subdomain_part, fqdn);
    }
}

/// Recursively insert a subdomain into a tree node
fn insert_into_node(node: &mut TreeNode, path: &str, _fqdn: &str) {
    let parts: Vec<&str> = path.split('.').collect();

    if parts.is_empty() {
        return;
    }

    if parts.len() == 1 {
        // Direct child
        let child_name = parts[0];
        node.children
            .entry(child_name.to_string())
            .or_insert_with(|| TreeNode::new(child_name.to_string(), "subdomain".to_string()));
    } else {
        // Nested child - recurse
        let first = parts[0];
        node.children
            .entry(first.to_string())
            .or_insert_with(|| TreeNode::new(first.to_string(), "subdomain".to_string()));

        if let Some(child) = node.children.get_mut(first) {
            let remaining = parts[1..].join(".");
            insert_into_node(child, &remaining, _fqdn);
        }
    }
}

/// Print tree structure recursively
fn print_tree_node(node: &TreeNode, prefix: &str, is_last: bool) {
    let connector = if is_last { "└── " } else { "├── " };
    let marker = match node.item_type.as_str() {
        "domain" => "🔵 ",   // Blue circle for root domains
        "subdomain" => "◇ ", // Diamond for subdomains
        _ => "○ ",
    };

    println!("{}{}{}{}", prefix, connector, marker, node.name);

    let child_count = node.children.len();
    for (idx, (_, child)) in node.children.iter().enumerate() {
        let is_last_child = idx == child_count - 1;
        let extension = if is_last { "    " } else { "│   " };
        print_tree_node(child, &format!("{}{}", prefix, extension), is_last_child);
    }
}

/// Display all domains and subdomains in a hierarchical tree structure
pub fn display_domain_tree(storage: &JsonlStorage) -> Result<(), io::Error> {
    let roots = build_domain_tree(storage)?;

    if roots.is_empty() {
        println!("No domains found in inventory.");
        return Ok(());
    }

    println!("Domain Hierarchy ({})", roots.len());
    println!();

    for (idx, root) in roots.iter().enumerate() {
        let is_last = idx == roots.len() - 1;
        print_tree_node(root, "", is_last);
    }

    Ok(())
}
