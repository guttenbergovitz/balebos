//! Provider-neutral external network service intelligence and offline
//! Censys / Shodan parsers (ADRs 0026, 0027, 0029).
//!
//! Port numbers are treated strictly as hypotheses; only protocol banners,
//! fingerprints, or authorised protocol checks corroborate protocol identity.
//! Provider timestamps (`source_recorded_at`) are never conflated with
//! `ingested_at` or live Balebos checks.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::normalize::normalize_fqdn;
use crate::domain::service::{
    ApplicationProtocol, EndpointFingerprint, FingerprintDiscriminatingPower, FingerprintKind,
    ProtocolIdentification, ServiceCategory, ServiceObservation, ServiceObservationState,
    SocketEndpoint, TransportProtocol, VerificationStatus,
};
use crate::error::{BalebosError, Result};
use crate::inventory::ItemType;

/// Provider-neutral external service observation input contract (ADR 0027).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalServiceInput {
    pub ip: String,
    pub port: u16,
    pub transport: TransportProtocol,
    #[serde(default)]
    pub hostnames: Vec<String>,
    #[serde(default)]
    pub reported_protocol: Option<String>,
    #[serde(default)]
    pub banner: Option<String>,
    #[serde(default)]
    pub product: Option<String>,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub source_recorded_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub tls_cert_sha256: Option<String>,
    #[serde(default)]
    pub ssh_host_key: Option<String>,
    #[serde(default)]
    pub http_title: Option<String>,
    #[serde(default)]
    pub http_favicon_hash: Option<String>,
    #[serde(default)]
    pub http_headers: Vec<(String, String)>,
    #[serde(default)]
    pub auth_required: Option<bool>,
    #[serde(default)]
    pub state: Option<ServiceObservationState>,
}

/// Infer a tentative protocol hypothesis from a well-known port number.
/// Never authoritative on its own.
pub fn port_hypothesis(port: u16, transport: TransportProtocol) -> Option<ApplicationProtocol> {
    match (port, transport) {
        (21, TransportProtocol::Tcp) => Some(ApplicationProtocol::Ftp),
        (22, TransportProtocol::Tcp) => Some(ApplicationProtocol::Ssh),
        (25 | 587 | 465, TransportProtocol::Tcp) => Some(ApplicationProtocol::Smtp),
        (53, _) => Some(ApplicationProtocol::Dns),
        (80 | 8080 | 8000, TransportProtocol::Tcp) => Some(ApplicationProtocol::Http),
        (110 | 995, TransportProtocol::Tcp) => Some(ApplicationProtocol::Pop3),
        (143 | 993, TransportProtocol::Tcp) => Some(ApplicationProtocol::Imap),
        (443 | 8443, TransportProtocol::Tcp) => Some(ApplicationProtocol::Https),
        (1433, TransportProtocol::Tcp) => Some(ApplicationProtocol::Mssql),
        (1883 | 8883, TransportProtocol::Tcp) => Some(ApplicationProtocol::Mqtt),
        (3306, TransportProtocol::Tcp) => Some(ApplicationProtocol::Mysql),
        (5432, TransportProtocol::Tcp) => Some(ApplicationProtocol::Postgresql),
        (5672 | 5671, TransportProtocol::Tcp) => Some(ApplicationProtocol::Amqp),
        (6379, TransportProtocol::Tcp) => Some(ApplicationProtocol::Redis),
        (9092, TransportProtocol::Tcp) => Some(ApplicationProtocol::Kafka),
        (9200 | 9300, TransportProtocol::Tcp) => Some(ApplicationProtocol::Elasticsearch),
        (27017, TransportProtocol::Tcp) => Some(ApplicationProtocol::Mongodb),
        _ => None,
    }
}

/// Parse a protocol label from external service index metadata or banners.
pub fn parse_protocol_label(raw: &str) -> Option<ApplicationProtocol> {
    let lower = raw.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return None;
    }
    if lower.contains("postgres") {
        Some(ApplicationProtocol::Postgresql)
    } else if lower.contains("mysql") || lower.contains("mariadb") {
        Some(ApplicationProtocol::Mysql)
    } else if lower.contains("mongo") {
        Some(ApplicationProtocol::Mongodb)
    } else if lower.contains("redis") {
        Some(ApplicationProtocol::Redis)
    } else if lower.contains("elastic") || lower.contains("opensearch") {
        Some(ApplicationProtocol::Elasticsearch)
    } else if lower.contains("mssql") || lower.contains("sql server") {
        Some(ApplicationProtocol::Mssql)
    } else if lower.contains("mqtt") {
        Some(ApplicationProtocol::Mqtt)
    } else if lower.contains("amqp") || lower.contains("rabbitmq") {
        Some(ApplicationProtocol::Amqp)
    } else if lower.contains("kafka") {
        Some(ApplicationProtocol::Kafka)
    } else if lower.contains("https") {
        Some(ApplicationProtocol::Https)
    } else if lower.contains("http") {
        Some(ApplicationProtocol::Http)
    } else if lower.contains("ssh") {
        Some(ApplicationProtocol::Ssh)
    } else if lower.contains("smtp") {
        Some(ApplicationProtocol::Smtp)
    } else if lower.contains("imap") {
        Some(ApplicationProtocol::Imap)
    } else if lower.contains("pop3") {
        Some(ApplicationProtocol::Pop3)
    } else if lower.contains("ftps") {
        Some(ApplicationProtocol::Ftps)
    } else if lower.contains("ftp") {
        Some(ApplicationProtocol::Ftp)
    } else if lower.contains("dns") {
        Some(ApplicationProtocol::Dns)
    } else if lower.contains("vpn") || lower.contains("openvpn") || lower.contains("wireguard") {
        Some(ApplicationProtocol::Vpn)
    } else {
        Some(ApplicationProtocol::Other(lower))
    }
}

/// Classify characteristic public management interfaces while rejecting
/// generic login pages and generic web servers (ADR 0029).
pub fn classify_management_interface(
    title: Option<&str>,
    headers: &[(String, String)],
    banner: Option<&str>,
    favicon_hash: Option<&str>,
) -> Option<(String, Vec<EndpointFingerprint>)> {
    let title_lower = title.unwrap_or("").to_ascii_lowercase();
    let banner_lower = banner.unwrap_or("").to_ascii_lowercase();
    let header_blob: String = headers
        .iter()
        .map(|(k, v)| format!("{}: {}", k.to_ascii_lowercase(), v.to_ascii_lowercase()))
        .collect::<Vec<_>>()
        .join("\n");

    let mut fps = Vec::new();
    let mut matched_name: Option<&'static str> = None;

    if title_lower.contains("grafana") || header_blob.contains("x-grafana") {
        matched_name = Some("Grafana");
    } else if header_blob.contains("x-jenkins") || title_lower.contains("dashboard [jenkins]") {
        matched_name = Some("Jenkins");
    } else if title_lower.contains("kibana") || header_blob.contains("kbn-name") {
        matched_name = Some("Kibana");
    } else if title_lower.contains("opensearch dashboards") || header_blob.contains("osd-name") {
        matched_name = Some("OpenSearch Dashboards");
    } else if title_lower.contains("prometheus time series") || banner_lower.contains("prometheus")
    {
        matched_name = Some("Prometheus");
    } else if title_lower.contains("phpmyadmin") || header_blob.contains("phpmyadmin") {
        matched_name = Some("phpMyAdmin");
    } else if title_lower.contains("kubernetes dashboard") {
        matched_name = Some("Kubernetes Dashboard");
    }

    if let Some(name) = matched_name {
        fps.push(EndpointFingerprint {
            kind: FingerprintKind::ManagementInterfaceSignature,
            value: name.to_string(),
            power: FingerprintDiscriminatingPower::Moderate,
            detail: title.map(|t| format!("HTTP title: {}", t)),
        });
        if let Some(fav) = favicon_hash {
            fps.push(EndpointFingerprint {
                kind: FingerprintKind::HttpFaviconHash,
                value: fav.to_string(),
                power: FingerprintDiscriminatingPower::Moderate,
                detail: Some(format!("Favicon associated with {}", name)),
            });
        }
        return Some((name.to_string(), fps));
    }

    None
}

fn category_for_protocol(proto: Option<&ApplicationProtocol>, has_mgmt: bool) -> ServiceCategory {
    if has_mgmt {
        return ServiceCategory::ManagementInterface;
    }
    match proto {
        Some(
            ApplicationProtocol::Postgresql
            | ApplicationProtocol::Mysql
            | ApplicationProtocol::Mongodb
            | ApplicationProtocol::Redis
            | ApplicationProtocol::Elasticsearch
            | ApplicationProtocol::Mssql,
        ) => ServiceCategory::Database,
        Some(
            ApplicationProtocol::Mqtt | ApplicationProtocol::Amqp | ApplicationProtocol::Kafka,
        ) => ServiceCategory::Messaging,
        _ => ServiceCategory::NetworkService,
    }
}

fn is_target_in_scope(ip: &str, hostnames: &[String], approved_scope: &[String]) -> bool {
    if approved_scope.is_empty() {
        return false;
    }
    let norm_scope: BTreeSet<String> = approved_scope
        .iter()
        .map(|s| {
            let trimmed = s.trim().to_ascii_lowercase();
            if trimmed.parse::<std::net::IpAddr>().is_ok() {
                trimmed
            } else {
                normalize_fqdn(&trimmed)
            }
        })
        .collect();
    if norm_scope.contains(&ip.trim().to_ascii_lowercase()) {
        return true;
    }
    hostnames.iter().any(|h| {
        let fqdn = normalize_fqdn(h);
        norm_scope
            .iter()
            .any(|scope| fqdn == *scope || fqdn.ends_with(&format!(".{}", scope)))
    })
}

/// Convert a provider-neutral [`ExternalServiceInput`] into a persisted
/// [`ServiceObservation`] with explicit protocol, fingerprint, and scope semantics.
pub fn normalise_external_service(
    input: ExternalServiceInput,
    approved_scope: &[String],
    ingested_at: DateTime<Utc>,
) -> ServiceObservation {
    let endpoint = SocketEndpoint::new(&input.ip, input.port, input.transport);
    let guessed = port_hypothesis(input.port, input.transport);

    let indexed_proto = input
        .reported_protocol
        .as_deref()
        .and_then(parse_protocol_label)
        .or_else(|| input.product.as_deref().and_then(parse_protocol_label));

    let banner_text = input
        .banner
        .clone()
        .or_else(|| input.product.clone())
        .or_else(|| input.reported_protocol.clone())
        .unwrap_or_default();

    let protocol_id = match (&guessed, &indexed_proto) {
        (Some(g), Some(obs)) if g != obs => {
            // HTTP on 443/8443 is compatible with HTTPS; otherwise flag mismatch.
            let compatible_http_tls = matches!(
                (g, obs),
                (ApplicationProtocol::Https, ApplicationProtocol::Http)
                    | (ApplicationProtocol::Http, ApplicationProtocol::Https)
            );
            if compatible_http_tls {
                ProtocolIdentification::IndexedBanner {
                    protocol: obs.clone(),
                    banner: banner_text.clone(),
                }
            } else {
                ProtocolIdentification::Mismatch {
                    port_implies: g.clone(),
                    observed: obs.clone(),
                    evidence: format!(
                        "Port {} suggests {}, but {} reported {}",
                        input.port, g, input.source, banner_text
                    ),
                }
            }
        }
        (_, Some(obs)) => ProtocolIdentification::IndexedBanner {
            protocol: obs.clone(),
            banner: banner_text.clone(),
        },
        (Some(g), None) => ProtocolIdentification::PortHypothesis { guessed: g.clone() },
        (None, None) => ProtocolIdentification::Unidentified,
    };

    let mut fingerprints = Vec::new();
    if let Some(cert) = &input.tls_cert_sha256 {
        fingerprints.push(EndpointFingerprint {
            kind: FingerprintKind::TlsCertSha256,
            value: cert.to_ascii_lowercase(),
            power: FingerprintDiscriminatingPower::Moderate,
            detail: Some("TLS leaf certificate SHA-256".to_string()),
        });
    }
    if let Some(ssh_key) = &input.ssh_host_key {
        fingerprints.push(EndpointFingerprint {
            kind: FingerprintKind::SshHostKey,
            value: ssh_key.clone(),
            power: FingerprintDiscriminatingPower::Discriminating,
            detail: Some("SSH server host public-key fingerprint".to_string()),
        });
    }
    let mgmt = classify_management_interface(
        input.http_title.as_deref(),
        &input.http_headers,
        input.banner.as_deref(),
        input.http_favicon_hash.as_deref(),
    );
    let interface_name = mgmt.as_ref().map(|(name, _)| name.clone());
    if let Some((_, mgmt_fps)) = mgmt {
        fingerprints.extend(mgmt_fps);
    } else if let Some(fav) = &input.http_favicon_hash {
        fingerprints.push(EndpointFingerprint {
            kind: FingerprintKind::HttpFaviconHash,
            value: fav.clone(),
            power: FingerprintDiscriminatingPower::Moderate,
            detail: None,
        });
    }

    if let Some(prod) = &input.product {
        let lower = prod.to_ascii_lowercase();
        if lower.contains("nginx") || lower.contains("apache") || lower.contains("iis") {
            fingerprints.push(EndpointFingerprint {
                kind: FingerprintKind::GenericBanner,
                value: prod.clone(),
                power: FingerprintDiscriminatingPower::Generic,
                detail: Some("Generic web server banner (weak signal)".to_string()),
            });
        }
    }

    let category = category_for_protocol(protocol_id.protocol(), interface_name.is_some());
    let in_scope = is_target_in_scope(&input.ip, &input.hostnames, approved_scope);
    let mut evidence = vec![format!(
        "Indexed by {} for {}",
        input.source,
        endpoint.canonical_id()
    )];
    if !banner_text.is_empty() {
        evidence.push(format!("Reported banner/protocol: {}", banner_text));
    }

    let uncertainty = match &protocol_id {
        ProtocolIdentification::PortHypothesis { guessed } => Some(format!(
            "Port {} suggests {} only as a hypothesis; not independently verified",
            input.port, guessed
        )),
        ProtocolIdentification::Mismatch {
            port_implies,
            observed,
            ..
        } => Some(format!(
            "Port {} implies {}, but observed/indexed protocol is {}",
            input.port, port_implies, observed
        )),
        _ => Some(
            "External index observation; not independently verified live by Balebos".to_string(),
        ),
    };

    let associated_hosts: Vec<String> = input
        .hostnames
        .into_iter()
        .map(|h| normalize_fqdn(&h))
        .filter(|h| !h.is_empty())
        .collect();

    ServiceObservation {
        item_type: ItemType::ServiceObservation,
        id: format!(
            "svc-{}-{}",
            input.source,
            endpoint.canonical_id().replace([':', '/'], "_")
        ),
        endpoint,
        host_ip: Some(input.ip.trim().to_ascii_lowercase()),
        associated_hosts,
        category,
        protocol_id,
        state: input.state.unwrap_or(ServiceObservationState::Open),
        verification: VerificationStatus::Indexed,
        source: input.source,
        source_recorded_at: input.source_recorded_at,
        ingested_at,
        fingerprints,
        interface_name,
        auth_required: input.auth_required,
        evidence,
        uncertainty,
        in_scope,
    }
}

/// Minimal Censys Hosts v2 JSON parser (supports either `{ "result": { ... } }`
/// or a single host object or array of host objects).
#[derive(Debug, Deserialize)]
struct CensysWrapper {
    #[serde(default)]
    result: Option<CensysHost>,
}

#[derive(Debug, Deserialize)]
struct CensysHost {
    ip: String,
    #[serde(default)]
    dns: Option<CensysDns>,
    #[serde(default)]
    services: Vec<CensysService>,
}

#[derive(Debug, Deserialize)]
struct CensysDns {
    #[serde(default)]
    names: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CensysService {
    port: u16,
    #[serde(default)]
    transport_protocol: Option<String>,
    #[serde(default)]
    service_name: Option<String>,
    #[serde(default)]
    extended_service_name: Option<String>,
    #[serde(default)]
    observed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    banner: Option<String>,
    #[serde(default)]
    tls: Option<CensysTls>,
    #[serde(default)]
    ssh: Option<CensysSsh>,
    #[serde(default)]
    http: Option<CensysHttp>,
}

#[derive(Debug, Deserialize)]
struct CensysTls {
    #[serde(default)]
    certificates: Option<CensysCertificates>,
}

#[derive(Debug, Deserialize)]
struct CensysCertificates {
    #[serde(default)]
    leaf_data: Option<CensysLeafData>,
}

#[derive(Debug, Deserialize)]
struct CensysLeafData {
    #[serde(default)]
    fingerprint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CensysSsh {
    #[serde(default)]
    server_host_key: Option<CensysSshKey>,
}

#[derive(Debug, Deserialize)]
struct CensysSshKey {
    #[serde(default)]
    fingerprint_sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CensysHttp {
    #[serde(default)]
    response: Option<CensysHttpResponse>,
}

#[derive(Debug, Deserialize)]
struct CensysHttpResponse {
    #[serde(default)]
    html_title: Option<String>,
    #[serde(default)]
    headers: BTreeMap<String, Vec<String>>,
}

/// Parse Censys Hosts v2 JSON fixture or response into [`ServiceObservation`]s.
pub fn parse_censys_host_json(
    json: &str,
    approved_scope: &[String],
    ingested_at: DateTime<Utc>,
) -> Result<Vec<ServiceObservation>> {
    let hosts: Vec<CensysHost> = if let Ok(w) = serde_json::from_str::<CensysWrapper>(json) {
        if let Some(h) = w.result {
            vec![h]
        } else if let Ok(h) = serde_json::from_str::<CensysHost>(json) {
            vec![h]
        } else {
            return Err(BalebosError::Validation(
                "Censys JSON did not contain a host result".to_string(),
            ));
        }
    } else if let Ok(list) = serde_json::from_str::<Vec<CensysHost>>(json) {
        list
    } else {
        return Err(BalebosError::Validation(
            "Invalid Censys host JSON payload".to_string(),
        ));
    };

    let mut out = Vec::new();
    for host in hosts {
        let hostnames = host.dns.map(|d| d.names).unwrap_or_default();
        for svc in host.services {
            let transport = match svc
                .transport_protocol
                .as_deref()
                .unwrap_or("TCP")
                .to_ascii_lowercase()
                .as_str()
            {
                "udp" => TransportProtocol::Udp,
                _ => TransportProtocol::Tcp,
            };
            let tls_sha = svc
                .tls
                .and_then(|t| t.certificates)
                .and_then(|c| c.leaf_data)
                .and_then(|l| l.fingerprint);
            let ssh_key = svc
                .ssh
                .and_then(|s| s.server_host_key)
                .and_then(|k| k.fingerprint_sha256);
            let (http_title, http_headers) = match svc.http.and_then(|h| h.response) {
                Some(r) => {
                    let hdrs = r
                        .headers
                        .into_iter()
                        .flat_map(|(k, vals)| vals.into_iter().map(move |v| (k.clone(), v)))
                        .collect();
                    (r.html_title, hdrs)
                }
                None => (None, Vec::new()),
            };

            out.push(normalise_external_service(
                ExternalServiceInput {
                    ip: host.ip.clone(),
                    port: svc.port,
                    transport,
                    hostnames: hostnames.clone(),
                    reported_protocol: svc.extended_service_name.or(svc.service_name),
                    banner: svc.banner,
                    product: None,
                    source: "censys".to_string(),
                    source_recorded_at: svc.observed_at,
                    tls_cert_sha256: tls_sha,
                    ssh_host_key: ssh_key,
                    http_title,
                    http_favicon_hash: None,
                    http_headers,
                    auth_required: None,
                    state: Some(ServiceObservationState::Open),
                },
                approved_scope,
                ingested_at,
            ));
        }
    }
    Ok(out)
}

#[derive(Debug, Deserialize)]
struct ShodanHost {
    ip_str: String,
    #[serde(default)]
    hostnames: Vec<String>,
    #[serde(default)]
    data: Vec<ShodanBanner>,
}

#[derive(Debug, Deserialize)]
struct ShodanBanner {
    port: u16,
    #[serde(default)]
    transport: Option<String>,
    #[serde(default)]
    product: Option<String>,
    #[serde(default)]
    data: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    ssl: Option<ShodanSsl>,
    #[serde(default)]
    ssh: Option<ShodanSsh>,
    #[serde(default)]
    http: Option<ShodanHttp>,
}

#[derive(Debug, Deserialize)]
struct ShodanSsl {
    #[serde(default)]
    cert: Option<ShodanCert>,
}

#[derive(Debug, Deserialize)]
struct ShodanCert {
    #[serde(default)]
    fingerprint: Option<ShodanFingerprint>,
}

#[derive(Debug, Deserialize)]
struct ShodanFingerprint {
    #[serde(default)]
    sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ShodanSsh {
    #[serde(default)]
    fingerprint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ShodanHttp {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    favicon: Option<ShodanFavicon>,
}

#[derive(Debug, Deserialize)]
struct ShodanFavicon {
    #[serde(default)]
    hash: Option<i64>,
}

fn parse_shodan_ts(raw: Option<&str>) -> Option<DateTime<Utc>> {
    let s = raw?.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(naive.and_utc());
    }
    None
}

/// Parse Shodan `/shodan/host/{ip}` JSON fixture or response into [`ServiceObservation`]s.
pub fn parse_shodan_host_json(
    json: &str,
    approved_scope: &[String],
    ingested_at: DateTime<Utc>,
) -> Result<Vec<ServiceObservation>> {
    let host: ShodanHost = serde_json::from_str(json)
        .map_err(|e| BalebosError::Validation(format!("Invalid Shodan JSON payload: {}", e)))?;

    let mut out = Vec::new();
    for entry in host.data {
        let transport = match entry
            .transport
            .as_deref()
            .unwrap_or("tcp")
            .to_ascii_lowercase()
            .as_str()
        {
            "udp" => TransportProtocol::Udp,
            _ => TransportProtocol::Tcp,
        };
        let tls_sha = entry
            .ssl
            .and_then(|s| s.cert)
            .and_then(|c| c.fingerprint)
            .and_then(|f| f.sha256);
        let ssh_key = entry.ssh.and_then(|s| s.fingerprint);
        let (http_title, fav_hash) = match entry.http {
            Some(h) => (
                h.title,
                h.favicon.and_then(|f| f.hash).map(|n| n.to_string()),
            ),
            None => (None, None),
        };

        out.push(normalise_external_service(
            ExternalServiceInput {
                ip: host.ip_str.clone(),
                port: entry.port,
                transport,
                hostnames: host.hostnames.clone(),
                reported_protocol: entry.product.clone(),
                banner: entry.data,
                product: entry.product,
                source: "shodan".to_string(),
                source_recorded_at: parse_shodan_ts(entry.timestamp.as_deref()),
                tls_cert_sha256: tls_sha,
                ssh_host_key: ssh_key,
                http_title,
                http_favicon_hash: fav_hash,
                http_headers: Vec::new(),
                auth_required: None,
                state: Some(ServiceObservationState::Open),
            },
            approved_scope,
            ingested_at,
        ));
    }
    Ok(out)
}

/// Correlate and deduplicate service observations across multiple sources by
/// canonical socket endpoint (`ip:port/transport`), preserving provenance and
/// flagging cross-source protocol conflicts without erasing either claim.
pub fn correlate_service_observations(
    observations: &[ServiceObservation],
) -> Vec<ServiceObservation> {
    let mut grouped: BTreeMap<String, Vec<&ServiceObservation>> = BTreeMap::new();
    for obs in observations {
        grouped
            .entry(obs.endpoint.canonical_id())
            .or_default()
            .push(obs);
    }

    let mut result = Vec::new();
    for (_key, group) in grouped {
        let mut merged = group[0].clone();
        let mut sources = BTreeSet::new();
        let mut hosts = BTreeSet::new();
        let mut protos = BTreeSet::new();
        let mut evidence = BTreeSet::new();

        for obs in &group {
            sources.insert(obs.source.clone());
            for h in &obs.associated_hosts {
                hosts.insert(h.clone());
            }
            if let Some(p) = obs.protocol_id.protocol() {
                protos.insert(p.to_string());
            }
            for e in &obs.evidence {
                evidence.insert(e.clone());
            }
            for fp in &obs.fingerprints {
                if !merged.fingerprints.contains(fp) {
                    merged.fingerprints.push(fp.clone());
                }
            }
            if obs.in_scope {
                merged.in_scope = true;
            }
            if let Some(ts) = obs.source_recorded_at {
                merged.source_recorded_at =
                    Some(merged.source_recorded_at.map_or(ts, |prev| prev.max(ts)));
            }
            if obs.interface_name.is_some() && merged.interface_name.is_none() {
                merged.interface_name = obs.interface_name.clone();
                merged.category = ServiceCategory::ManagementInterface;
            }
        }

        if sources.len() > 1 {
            merged.source = sources.into_iter().collect::<Vec<_>>().join("+");
        }
        merged.associated_hosts = hosts.into_iter().collect();
        merged.evidence = evidence.into_iter().collect();
        if protos.len() > 1 {
            merged.uncertainty = Some(format!(
                "Conflicting protocol reports across sources: {}",
                protos.into_iter().collect::<Vec<_>>().join(" vs ")
            ));
        }
        result.push(merged);
    }
    result
}
