//! 22 deterministic offline benchmark scenarios covering external service
//! discovery, protocol separation, fingerprint correlation, application
//! references, scope isolation, persistence, and discovery metrics (Section 16).

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::{Duration, TimeZone, Utc};

use crate::app::external_services::{
    classify_management_interface, correlate_service_observations, parse_censys_host_json,
    parse_shodan_host_json,
};
use crate::app::metrics::{compute_discovery_metrics, CandidateReviewDecision};
use crate::app::probe::types::{DnsObservation, HttpObservation, TlsObservation};
use crate::app::references::{
    extract_dns_srv_txt_references, extract_js_references, parse_openapi_references,
    parse_public_repo_references, parse_urlscan_result_json,
};
use crate::app::relationships::{explain_connection, project_relationships};
use crate::domain::relationship::{EntityKind, RelationshipKind, SignalCategory};
use crate::domain::service::{
    ApplicationProtocol, ProtocolIdentification, ServiceCategory, ServiceObservationState,
    VerificationStatus,
};
use crate::inventory::{InventoryItem, ItemType};
use crate::storage::reader::Records;
use crate::storage::{SqliteStorage, StorageEngine, StorageEngineExt};

fn ts(day: u32) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, day, 12, 0, 0).unwrap()
}

fn seed_item(id: &str, name: &str) -> InventoryItem {
    InventoryItem {
        item_type: ItemType::Domain,
        id: id.to_string(),
        name: name.to_string(),
        original_input: Some(name.to_string()),
        discovered_at: ts(1),
        source: "cli".to_string(),
    }
}

fn dns_obs(id: &str, host: &str, ips: &[&str], day: u32) -> DnsObservation {
    DnsObservation {
        item_type: ItemType::DnsRecord,
        id: id.to_string(),
        asset_type: "domain".to_string(),
        target: host.to_string(),
        a: ips.iter().map(|s| s.to_string()).collect(),
        aaaa: Vec::new(),
        cname: Vec::new(),
        mx: Vec::new(),
        ns: Vec::new(),
        txt: Vec::new(),
        soa: Vec::new(),
        srv: Vec::new(),
        checked_at: ts(day),
        source: "dnsx".to_string(),
    }
}

#[test]
fn benchmark_scenarios_1_to_8_services_and_fingerprints() {
    let ingested = ts(23);

    // 1. Known domain resolving to an IP with an indexed PostgreSQL service.
    let censys_pg = r#"{
      "result": {
        "ip": "203.0.113.10",
        "dns": { "names": ["db.example.com"] },
        "services": [{
          "port": 5432,
          "transport_protocol": "TCP",
          "service_name": "POSTGRES",
          "observed_at": "2026-11-20T10:00:00Z",
          "banner": "PostgreSQL 16.2"
        }]
      }
    }"#;
    let pg = parse_censys_host_json(censys_pg, &["example.com".to_string()], ingested).unwrap();
    assert_eq!(pg.len(), 1);
    assert_eq!(pg[0].category, ServiceCategory::Database);
    assert_eq!(
        pg[0].protocol_id.protocol(),
        Some(&ApplicationProtocol::Postgresql)
    );
    assert_eq!(pg[0].verification, VerificationStatus::Indexed);
    assert_eq!(
        pg[0]
            .source_recorded_at
            .unwrap()
            .format("%Y-%m-%d")
            .to_string(),
        "2026-11-20"
    );

    // 2. Port 5432 exposing a non-PostgreSQL protocol (HTTP mismatch).
    let censys_mismatch = r#"{
      "result": {
        "ip": "203.0.113.11",
        "services": [{
          "port": 5432,
          "transport_protocol": "TCP",
          "service_name": "HTTP",
          "observed_at": "2026-11-20T10:00:00Z",
          "banner": "HTTP/1.1 200 OK"
        }]
      }
    }"#;
    let mismatch = parse_censys_host_json(censys_mismatch, &[], ingested).unwrap();
    assert!(matches!(
        mismatch[0].protocol_id,
        ProtocolIdentification::Mismatch {
            port_implies: ApplicationProtocol::Postgresql,
            observed: ApplicationProtocol::Http,
            ..
        }
    ));

    // 3. Historical database service no longer externally confirmed.
    let mut hist_pg = pg[0].clone();
    hist_pg.id = "svc-hist".to_string();
    hist_pg.state = ServiceObservationState::HistoricalOnly;
    let mut recs_hist = Records::default();
    recs_hist.service_observations.push(hist_pg);
    let hist_edges = project_relationships(&recs_hist);
    assert!(hist_edges
        .iter()
        .any(|e| e.kind == RelationshipKind::ExposesService && !e.is_current));

    // 4. Redis service indexed on an IP shared by several unrelated tenants.
    let shodan_redis = r#"{
      "ip_str": "203.0.113.20",
      "hostnames": ["tenant-a.example.com", "unrelated-b.example.org"],
      "data": [{
        "port": 6379,
        "transport": "tcp",
        "product": "Redis",
        "data": "+PONG",
        "timestamp": "2026-11-18T08:00:00Z"
      }]
    }"#;
    let redis =
        parse_shodan_host_json(shodan_redis, &["example.com".to_string()], ingested).unwrap();
    assert_eq!(redis[0].category, ServiceCategory::Database);
    let mut recs_shared = Records::default();
    recs_shared.service_observations.push(redis[0].clone());
    let shared_edges = project_relationships(&recs_shared);
    assert!(shared_edges
        .iter()
        .filter(|e| e.kind == RelationshipKind::ExposesService)
        .all(|e| e.signal != SignalCategory::OperatorConfirmed));

    // 5. Public management interface with characteristic HTTP fingerprint (Grafana).
    let mgmt = classify_management_interface(
        Some("Grafana - Metrics"),
        &[("X-Grafana-Org".to_string(), "1".to_string())],
        None,
        Some("123456"),
    );
    assert_eq!(mgmt.as_ref().map(|(n, _)| n.as_str()), Some("Grafana"));

    // 6. Generic login page incorrectly resembling a management interface.
    let generic_login = classify_management_interface(
        Some("Admin Login - Sign In"),
        &[("Server".to_string(), "nginx".to_string())],
        Some("<form><input type='password'></form>"),
        None,
    );
    assert!(generic_login.is_none());

    // 7. Two endpoints with a matching SSH host key.
    let ssh_a = parse_censys_host_json(
        r#"{"result":{"ip":"203.0.113.30","services":[{"port":22,"transport_protocol":"TCP","service_name":"SSH","ssh":{"server_host_key":{"fingerprint_sha256":"SHA256:abcdef123"}}}]}}"#,
        &[],
        ingested,
    )
    .unwrap();
    let ssh_b = parse_censys_host_json(
        r#"{"result":{"ip":"203.0.113.31","services":[{"port":22,"transport_protocol":"TCP","service_name":"SSH","ssh":{"server_host_key":{"fingerprint_sha256":"SHA256:abcdef123"}}}]}}"#,
        &[],
        ingested,
    )
    .unwrap();
    let mut recs_ssh = Records::default();
    recs_ssh.service_observations.extend(ssh_a);
    recs_ssh.service_observations.extend(ssh_b);
    let exp_ssh = explain_connection(&recs_ssh, "203.0.113.30:22/tcp", "203.0.113.31:22/tcp");
    assert!(exp_ssh.shared_paths.iter().any(|p| {
        p.left.kind == RelationshipKind::SharesSshHostKey
            && p.left.signal == SignalCategory::StrongCandidate
    }));

    // 8. Shared TLS certificate across unrelated tenants behind CDN remains WeakSharedClue.
    let mut recs_cdn = Records::default();
    recs_cdn.http_observations.push(HttpObservation {
        item_type: ItemType::HttpCheck,
        id: "h1".to_string(),
        asset_type: "domain".to_string(),
        target: "a.example.com".to_string(),
        status: Some(200),
        title: None,
        webserver: None,
        tech: Vec::new(),
        cdn_name: Some("Cloudflare".to_string()),
        favicon_hash: None,
        final_url: None,
        redirect_chain: Vec::new(),
        tls: Some(TlsObservation {
            subject_dn: "CN=*.cdn.example".to_string(),
            issuer_dn: "CN=CA".to_string(),
            not_before: ts(1),
            not_after: ts(1) + Duration::days(90),
            fingerprint_sha256: "deadbeef".to_string(),
        }),
        error_message: None,
        error_kind: None,
        response_time_ms: 10,
        checked_at: ts(20),
        source: "httpx".to_string(),
    });
    recs_cdn.http_observations.push(HttpObservation {
        id: "h2".to_string(),
        target: "other.example.org".to_string(),
        ..recs_cdn.http_observations[0].clone()
    });
    let rel_cdn = crate::app::relationships::related_entities(&recs_cdn, "a.example.com");
    assert_eq!(rel_cdn.shared_clues.len(), 1);
    assert_eq!(
        rel_cdn.shared_clues[0].signal,
        SignalCategory::WeakSharedClue
    );
}

#[test]
fn benchmark_scenarios_9_to_19_references_scope_and_dedup() {
    let ingested = ts(23);
    let scope = vec!["example.com".to_string()];

    // 9 & 10. JS reference to existing API and deprecated API (plus secret redaction).
    let js = r#"
      const api = "https://api.example.com/v2/users?token=supersecret123";
      // deprecated legacy endpoint
      const oldApi = "https://legacy-api.example.com/v1/items";
    "#;
    let js_refs = extract_js_references(
        "www.example.com",
        "https://www.example.com/assets/app.js",
        js,
        &scope,
        None,
        ingested,
    );
    assert!(js_refs
        .iter()
        .any(|r| r.referenced_target == "api.example.com"
            && r.relationship_kind == RelationshipKind::ReferencesApi
            && r.evidence[0].contains("[REDACTED]")));
    assert!(js_refs
        .iter()
        .any(|r| r.referenced_target == "legacy-api.example.com" && r.deprecated_hint));

    // 11. OpenAPI specification declaring multiple server environments.
    let openapi = r#"{
      "openapi": "3.0.3",
      "servers": [
        { "url": "https://api.example.com/v2", "description": "Production" },
        { "url": "https://staging-api.example.com/v2", "description": "Staging" }
      ]
    }"#;
    let oapi_refs = parse_openapi_references(
        "api.example.com",
        "https://api.example.com/openapi.json",
        openapi,
        &scope,
        None,
        ingested,
    )
    .unwrap();
    assert_eq!(oapi_refs.len(), 2);
    assert!(oapi_refs
        .iter()
        .all(|r| r.verification == VerificationStatus::Declared));
    assert!(oapi_refs
        .iter()
        .any(|r| r.referenced_target == "staging-api.example.com" && r.dev_or_placeholder_hint));

    // 12 & 13. DNS SRV record + TXT verification token suppression.
    let dns_refs = extract_dns_srv_txt_references(
        "example.com",
        &["10 5 5061 sip.example.com".to_string()],
        &[
            "google-site-verification=abc123opaque".to_string(),
            "MS=ms99887766".to_string(),
            "v=spf1 include:_spf.google.com ~all".to_string(),
        ],
        &scope,
        ingested,
    );
    assert!(dns_refs
        .iter()
        .any(|r| r.referenced_target == "sip.example.com:5061/tcp"));
    assert!(dns_refs
        .iter()
        .any(|r| r.referenced_target == "_spf.google.com"));
    assert!(!dns_refs
        .iter()
        .any(|r| r.referenced_target.contains("abc123opaque")
            || r.referenced_target.contains("ms99887766")));

    // 14. Public storage URL does not imply bucket-listing permission or ownership.
    let urlscan = r#"{
      "task": { "url": "https://www.example.com", "time": "2026-11-19T12:00:00Z" },
      "lists": {
        "urls": ["https://public-assets.s3.amazonaws.com/logo.png"],
        "domains": ["www.example.com", "cdn.thirdparty.net"]
      }
    }"#;
    let us_refs =
        parse_urlscan_result_json("www.example.com", urlscan, false, &scope, ingested).unwrap();
    let s3 = us_refs
        .iter()
        .find(|r| r.target_kind == EntityKind::StorageEndpoint)
        .unwrap();
    assert!(s3.uncertainty.as_deref().unwrap().contains("ownership"));
    assert_eq!(s3.source, "urlscan_io_indexed");

    // 15. Public repository configuration containing a development-only endpoint.
    let tf =
        r#"resource "aws_ssm_parameter" "api" { value = "https://dev-internal.example.com/api" }"#;
    let repo_refs = parse_public_repo_references(
        "example.com",
        "infra/dev/main.tf",
        tf,
        &scope,
        None,
        ingested,
    );
    assert_eq!(repo_refs.len(), 1);
    assert_eq!(repo_refs[0].verification, VerificationStatus::Declared);
    assert!(repo_refs[0].dev_or_placeholder_hint);

    // 16. Reference pointing outside approved active scope.
    let out_of_scope = us_refs
        .iter()
        .find(|r| r.referenced_target == "cdn.thirdparty.net")
        .unwrap();
    assert!(!out_of_scope.in_scope);

    // 17, 18, 19. Stale timestamps, conflicting observations, and duplicate endpoint correlation.
    let censys_stale = parse_censys_host_json(
        r#"{"result":{"ip":"203.0.113.50","services":[{"port":6379,"transport_protocol":"TCP","service_name":"REDIS","observed_at":"2025-01-10T00:00:00Z"}]}}"#,
        &[],
        ingested,
    )
    .unwrap();
    let shodan_conflict = parse_shodan_host_json(
        r#"{"ip_str":"203.0.113.50","data":[{"port":6379,"transport":"tcp","product":"HTTP","timestamp":"2026-11-21T00:00:00Z"}]}"#,
        &[],
        ingested,
    )
    .unwrap();
    assert_eq!(
        censys_stale[0]
            .source_recorded_at
            .unwrap()
            .format("%Y-%m-%d")
            .to_string(),
        "2025-01-10"
    );
    let combined = [censys_stale, shodan_conflict].concat();
    let correlated = correlate_service_observations(&combined);
    assert_eq!(correlated.len(), 1);
    assert_eq!(correlated[0].source, "censys+shodan");
    assert!(correlated[0]
        .uncertainty
        .as_deref()
        .unwrap()
        .contains("Conflicting protocol"));
}

#[test]
fn benchmark_scenarios_20_to_22_persistence_disappearance_and_diminishing_yield() {
    // 20. Candidate persistence and recovery after restarting Balebos.
    let dir = std::env::temp_dir().join(format!("balebos_bench_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("balebos.db");
    let db_str = db_path.to_str().unwrap();

    let censys = parse_censys_host_json(
        r#"{"result":{"ip":"203.0.113.70","dns":{"names":["api.example.com"]},"services":[{"port":5432,"transport_protocol":"TCP","service_name":"POSTGRES","observed_at":"2026-11-22T00:00:00Z"}]}}"#,
        &["example.com".to_string()],
        ts(23),
    )
    .unwrap();
    let js_refs = extract_js_references(
        "www.example.com",
        "https://www.example.com/app.js",
        "fetch('https://api.example.com/v1/health')",
        &["example.com".to_string()],
        None,
        ts(23),
    );

    {
        let storage: Arc<dyn StorageEngine> = Arc::new(SqliteStorage::new(db_str).unwrap());
        storage.save(&seed_item("i1", "api.example.com")).unwrap();
        storage
            .save(&dns_obs("d1", "api.example.com", &["203.0.113.70"], 22))
            .unwrap();
        storage.save(&censys[0]).unwrap();
        storage.save(&js_refs[0]).unwrap();
    }

    let reopened = SqliteStorage::new(db_str).unwrap();
    let records = reopened.read_all_records().unwrap();
    assert_eq!(records.service_observations.len(), 1);
    assert_eq!(records.application_references.len(), 1);

    let mut reviews = BTreeMap::new();
    reviews.insert(
        "203.0.113.70:5432/tcp".to_string(),
        CandidateReviewDecision::Confirmed,
    );
    let metrics = compute_discovery_metrics(&records, &[], &[], &reviews);
    assert_eq!(metrics.service_discovery.unique_service_endpoints, 1);
    assert_eq!(metrics.service_discovery.corroborated_protocols, 1);
    assert_eq!(metrics.service_discovery.api_endpoints, 1);
    assert_eq!(
        metrics
            .service_discovery
            .per_category_reviewed_precision
            .get("database")
            .copied()
            .flatten(),
        Some(1.0)
    );

    // 21. Service disappearance with incomplete scan coverage does not claim deletion.
    let mut partial_records = reopened.read_all_records().unwrap();
    partial_records.service_observations[0].state = ServiceObservationState::HistoricalOnly;
    let partial_metrics = compute_discovery_metrics(&partial_records, &[], &[], &reviews);
    assert_eq!(partial_metrics.service_discovery.historical_only_count, 1);

    // 22. Repeated identical scan yields zero additional unique service endpoints.
    let mut repeated = reopened.read_all_records().unwrap();
    repeated.service_observations.push(censys[0].clone());
    let repeated_metrics = compute_discovery_metrics(&repeated, &[], &[], &reviews);
    assert_eq!(
        repeated_metrics.service_discovery.unique_service_endpoints,
        1
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn utf8_multibyte_boundaries_do_not_panic_in_reference_extraction() {
    let prefix = "ąęółżźćń🔐".repeat(6);
    let js = format!(
        "{} const api = \"https://utf8-api.example.com/v1\"; {}",
        prefix, prefix
    );
    let refs = extract_js_references(
        "www.example.com",
        "https://www.example.com/app.js",
        &js,
        &["example.com".to_string()],
        None,
        ts(23),
    );
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].referenced_target, "utf8-api.example.com");

    let large_prefix = "ą".repeat(600 * 1024);
    let large_js = format!("{} https://after-limit.example.com", large_prefix);
    let bounded_refs = extract_js_references(
        "www.example.com",
        "https://www.example.com/big.js",
        &large_js,
        &[],
        None,
        ts(23),
    );
    assert!(bounded_refs.is_empty());
}

#[test]
fn service_correlation_keeps_latest_observation_state_and_protocol() {
    let older = parse_censys_host_json(
        r#"{"result":{"ip":"203.0.113.80","services":[{"port":5432,"transport_protocol":"TCP","service_name":"POSTGRES","observed_at":"2026-11-10T00:00:00Z"}]}}"#,
        &[],
        ts(11),
    )
    .unwrap();
    let mut older_hist = older[0].clone();
    older_hist.state = ServiceObservationState::HistoricalOnly;

    let newer = parse_shodan_host_json(
        r#"{"ip_str":"203.0.113.80","data":[{"port":5432,"transport":"tcp","product":"PostgreSQL","timestamp":"2026-11-22T00:00:00Z"}]}"#,
        &[],
        ts(23),
    )
    .unwrap();

    // Pass older first, newer second: latest state (`Open`) must win!
    let correlated = correlate_service_observations(&[older_hist, newer[0].clone()]);
    assert_eq!(correlated.len(), 1);
    assert_eq!(correlated[0].state, ServiceObservationState::Open);
    assert_eq!(
        correlated[0]
            .source_recorded_at
            .unwrap()
            .format("%Y-%m-%d")
            .to_string(),
        "2026-11-22"
    );
}

#[test]
fn novelty_metrics_exclude_references_already_in_inventory() {
    let mut records = Records::default();
    records.items.push(seed_item("i1", "api.example.com"));
    let refs = extract_js_references(
        "www.example.com",
        "https://www.example.com/app.js",
        "fetch('https://api.example.com/v1'); fetch('https://new-svc.example.com/v1');",
        &["example.com".to_string()],
        None,
        ts(23),
    );
    records.application_references.extend(refs);
    let report = compute_discovery_metrics(&records, &[], &[], &BTreeMap::new());
    // `api.example.com` is already in `records.items`, so only `new-svc.example.com` is novel.
    assert_eq!(report.total_unique_candidates, 1);
}
