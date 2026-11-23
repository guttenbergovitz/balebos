//! Role-separated infrastructure attribution engine.
//!
//! Collects all available signals (CDN/edge, CNAME, NS delegation, ASN,
//! HTTP headers, RDAP registrar) and attributes each infrastructure role
//! independently without conflating intermediaries (such as CloudFront
//! or Cloudflare) with origin hosting.

use super::providers::{Provider, ProviderCategory, ProviderKnowledgeBase};
use crate::app::probe::{
    AsnObservation, Confidence, DetectionMethod, DnsObservation, HostingObservation,
    HttpObservation,
};
use crate::inventory::ItemType;
use crate::org::DomainRegistrationRecord;
use crate::storage::{StorageEngine, StorageEngineExt};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Explicit verification state for an attribution (never a statistical probability).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributionStatus {
    DirectlyObserved,
    SupportedAttribution,
    HeuristicCandidate,
    ConflictingEvidence,
    InsufficientEvidence,
    NotExternallyObservable,
}

impl std::fmt::Display for AttributionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DirectlyObserved => write!(f, "Directly observed"),
            Self::SupportedAttribution => write!(f, "Supported attribution"),
            Self::HeuristicCandidate => write!(f, "Heuristic candidate"),
            Self::ConflictingEvidence => write!(f, "Conflicting evidence"),
            Self::InsufficientEvidence => write!(f, "Insufficient evidence"),
            Self::NotExternallyObservable => write!(f, "Not externally observable"),
        }
    }
}

/// Registrar attribution from authoritative RDAP or WHOIS fallback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistrarAttribution {
    pub name: Option<String>,
    pub iana_registrar_id: Option<String>,
    pub source: String,
    pub completeness: Option<String>,
}

/// Attribution of a specific provider role (DNS, CDN/edge, or PaaS).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderAttribution {
    pub provider_id: Option<String>,
    pub provider_name: String,
    pub status: AttributionStatus,
    pub evidence: Vec<String>,
}

/// Observed network operator for an IP address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkAttribution {
    pub ip: String,
    pub asn: u32,
    pub org: String,
    pub provider_hint: Option<String>,
}

/// Origin hosting attribution kept strictly separate from CDN/edge intermediaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OriginAttribution {
    Observed {
        provider_id: String,
        provider_name: String,
        provider_category: String,
        status: AttributionStatus,
        evidence: Vec<String>,
    },
    Conflicting {
        candidates: Vec<String>,
        evidence: Vec<String>,
    },
    Unknown {
        reason: String,
        evidence: Vec<String>,
    },
}

/// Role-separated infrastructure attribution for a single target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfrastructureAttribution {
    pub target: String,
    pub registrar: Option<RegistrarAttribution>,
    pub authoritative_dns: Vec<ProviderAttribution>,
    pub cdn: Vec<ProviderAttribution>,
    pub network: Vec<NetworkAttribution>,
    pub application_platform: Vec<ProviderAttribution>,
    pub origin: OriginAttribution,
    pub rule_version: &'static str,
}

/// Hosting detection engine.
pub struct HostingDetector {
    kb: ProviderKnowledgeBase,
}

/// Input signals for hosting detection.
pub struct DetectionInput<'a> {
    pub target: &'a str,
    pub dns: Option<&'a DnsObservation>,
    pub http: Option<&'a HttpObservation>,
    pub asn_by_ip: &'a HashMap<String, AsnObservation>,
}

/// Intermediate detection result before being stored as legacy `HostingObservation`.
#[derive(Debug, Clone)]
pub(crate) struct DetectionResult {
    provider_id: Option<String>,
    provider_name: Option<String>,
    provider_category: Option<String>,
    cdn_detected: bool,
    cdn_name: Option<String>,
    confidence: Confidence,
    detection_method: DetectionMethod,
    evidence: Vec<String>,
}

impl HostingDetector {
    /// Create a new detector with the default knowledge base.
    pub fn new() -> Self {
        Self {
            kb: ProviderKnowledgeBase::load(),
        }
    }

    /// Create a detector with a custom knowledge base.
    #[allow(dead_code)]
    pub fn with_knowledge_base(kb: ProviderKnowledgeBase) -> Self {
        Self { kb }
    }

    /// Produce a role-separated `InfrastructureAttribution` by collecting all
    /// available signals without early-return short-circuiting.
    pub fn attribute(
        &self,
        input: &DetectionInput,
        registration: Option<&DomainRegistrationRecord>,
    ) -> InfrastructureAttribution {
        let mut authoritative_dns = Vec::new();
        let mut cdn = Vec::new();
        let mut network = Vec::new();
        let mut application_platform = Vec::new();
        let mut origin_candidates: Vec<(&Provider, String)> = Vec::new();
        let mut all_evidence = Vec::new();

        // 1. Registrar
        let registrar = registration.map(|reg| RegistrarAttribution {
            name: reg.registrar.clone(),
            iana_registrar_id: reg.iana_registrar_id.clone(),
            source: reg.source.clone(),
            completeness: reg.completeness.clone(),
        });

        // 2. Authoritative DNS (from NS delegation)
        if let Some(dns) = input.dns {
            for ns in &dns.ns {
                let matches = self.kb.find_by_cname(ns);
                if matches.is_empty() {
                    authoritative_dns.push(ProviderAttribution {
                        provider_id: None,
                        provider_name: ns.clone(),
                        status: AttributionStatus::DirectlyObserved,
                        evidence: vec![format!("NS delegation: {ns}")],
                    });
                } else {
                    for p in matches {
                        if !authoritative_dns
                            .iter()
                            .any(|a| a.provider_id.as_deref() == Some(p.id.as_str()))
                        {
                            authoritative_dns.push(ProviderAttribution {
                                provider_id: Some(p.id.clone()),
                                provider_name: p.name.clone(),
                                status: AttributionStatus::SupportedAttribution,
                                evidence: vec![format!("NS {ns} matches {}", p.name)],
                            });
                        }
                    }
                }
            }
        }

        // 3. CDN / Edge signals from HTTP `cdn_name`
        if let Some(http) = input.http {
            if let Some(cdn_name) = &http.cdn_name {
                let matches = self.kb.find_by_cdn_name(cdn_name);
                if matches.is_empty() {
                    let ev = format!("CDN detected: {cdn_name} (not in KB)");
                    all_evidence.push(ev.clone());
                    cdn.push(ProviderAttribution {
                        provider_id: None,
                        provider_name: cdn_name.clone(),
                        status: AttributionStatus::DirectlyObserved,
                        evidence: vec![ev],
                    });
                } else {
                    for p in matches {
                        if p.id == "aws" && cdn_name.to_lowercase().contains("cloudfront") {
                            let ev = format!(
                                "CDN {cdn_name} identifies AWS CloudFront edge (origin masked)"
                            );
                            all_evidence.push(ev.clone());
                            cdn.push(ProviderAttribution {
                                provider_id: Some("cloudfront".to_string()),
                                provider_name: "CloudFront".to_string(),
                                status: AttributionStatus::DirectlyObserved,
                                evidence: vec![ev],
                            });
                        } else if p.is_cdn() {
                            let ev = format!("Behind CDN: {}", p.name);
                            all_evidence.push(ev.clone());
                            cdn.push(ProviderAttribution {
                                provider_id: Some(p.id.clone()),
                                provider_name: p.name.clone(),
                                status: AttributionStatus::DirectlyObserved,
                                evidence: vec![ev],
                            });
                        } else if p.category == ProviderCategory::Edge {
                            let ev = format!("Platform edge signature: {}", p.name);
                            all_evidence.push(ev.clone());
                            application_platform.push(ProviderAttribution {
                                provider_id: Some(p.id.clone()),
                                provider_name: p.name.clone(),
                                status: AttributionStatus::SupportedAttribution,
                                evidence: vec![ev],
                            });
                        }
                    }
                }
            }
        }

        // 4. CNAME signals (label-boundary matched)
        if let Some(dns) = input.dns {
            for cname in &dns.cname {
                let matches = self.kb.find_by_cname(cname);
                for p in matches {
                    let ev = format!("CNAME {cname} matches {}", p.name);
                    all_evidence.push(ev.clone());
                    if p.id == "aws" && cname.to_lowercase().contains("cloudfront.net") {
                        if !cdn.iter().any(|c| c.provider_name == "CloudFront") {
                            cdn.push(ProviderAttribution {
                                provider_id: Some("cloudfront".to_string()),
                                provider_name: "CloudFront".to_string(),
                                status: AttributionStatus::DirectlyObserved,
                                evidence: vec![ev],
                            });
                        }
                    } else if p.is_cdn() {
                        if !cdn
                            .iter()
                            .any(|c| c.provider_id.as_deref() == Some(p.id.as_str()))
                        {
                            cdn.push(ProviderAttribution {
                                provider_id: Some(p.id.clone()),
                                provider_name: p.name.clone(),
                                status: AttributionStatus::SupportedAttribution,
                                evidence: vec![ev],
                            });
                        }
                    } else if p.category == ProviderCategory::Edge {
                        if !application_platform
                            .iter()
                            .any(|a| a.provider_id.as_deref() == Some(p.id.as_str()))
                        {
                            application_platform.push(ProviderAttribution {
                                provider_id: Some(p.id.clone()),
                                provider_name: p.name.clone(),
                                status: AttributionStatus::SupportedAttribution,
                                evidence: vec![ev],
                            });
                        }
                    } else {
                        origin_candidates.push((p, ev));
                    }
                }
            }
        }

        // 5. Network operator signals (ASN)
        if let Some(dns) = input.dns {
            for ip in dns.a.iter().chain(dns.aaaa.iter()) {
                if let Some(asn_obs) = input.asn_by_ip.get(ip) {
                    if asn_obs.asn == 0 {
                        continue;
                    }
                    let matches = self.kb.find_by_asn(asn_obs.asn);
                    let hint = matches.first().map(|p| p.name.clone());
                    network.push(NetworkAttribution {
                        ip: ip.clone(),
                        asn: asn_obs.asn,
                        org: asn_obs.org.clone(),
                        provider_hint: hint,
                    });
                    if matches.is_empty() {
                        all_evidence.push(format!(
                            "IP {ip} ASN {} ({}) - unrecognised network operator",
                            asn_obs.asn, asn_obs.org
                        ));
                    } else {
                        for p in matches {
                            let ev = format!(
                                "IP {ip} ASN {} ({}) matches {}",
                                asn_obs.asn, asn_obs.org, p.name
                            );
                            all_evidence.push(ev.clone());
                            if p.is_cdn() {
                                if !cdn
                                    .iter()
                                    .any(|c| c.provider_id.as_deref() == Some(p.id.as_str()))
                                {
                                    cdn.push(ProviderAttribution {
                                        provider_id: Some(p.id.clone()),
                                        provider_name: p.name.clone(),
                                        status: AttributionStatus::HeuristicCandidate,
                                        evidence: vec![ev],
                                    });
                                }
                            } else {
                                origin_candidates.push((p, ev));
                            }
                        }
                    }
                }
            }
        }

        // 6. Platform-specific HTTP headers (never generic server software)
        if let Some(http) = input.http {
            if let Some(webserver) = &http.webserver {
                for p in self.kb.find_by_header(webserver) {
                    let ev = format!("Header '{webserver}' matches {}", p.name);
                    all_evidence.push(ev.clone());
                    if p.category == ProviderCategory::Edge
                        && !application_platform
                            .iter()
                            .any(|a| a.provider_id.as_deref() == Some(p.id.as_str()))
                    {
                        application_platform.push(ProviderAttribution {
                            provider_id: Some(p.id.clone()),
                            provider_name: p.name.clone(),
                            status: AttributionStatus::HeuristicCandidate,
                            evidence: vec![ev],
                        });
                    }
                }
            }
        }

        // 7. Determine origin attribution without masking uncertainty
        let mut unique_origin_ids: Vec<&str> = Vec::new();
        for (p, _) in &origin_candidates {
            if !unique_origin_ids.contains(&p.id.as_str()) {
                unique_origin_ids.push(p.id.as_str());
            }
        }

        let origin = if !cdn.is_empty() {
            OriginAttribution::Unknown {
                reason: "Origin not externally observable behind CDN".to_string(),
                evidence: all_evidence,
            }
        } else if !application_platform.is_empty() && unique_origin_ids.is_empty() {
            OriginAttribution::Unknown {
                reason: "Managed application platform; underlying origin not directly observable"
                    .to_string(),
                evidence: all_evidence,
            }
        } else if unique_origin_ids.len() == 1 {
            let p = origin_candidates[0].0;
            let status = if origin_candidates
                .iter()
                .any(|(_, ev)| ev.starts_with("CNAME "))
            {
                AttributionStatus::SupportedAttribution
            } else {
                AttributionStatus::HeuristicCandidate
            };
            OriginAttribution::Observed {
                provider_id: p.id.clone(),
                provider_name: p.name.clone(),
                provider_category: p.category.to_string(),
                status,
                evidence: all_evidence,
            }
        } else if unique_origin_ids.len() > 1 {
            let names: Vec<String> = unique_origin_ids
                .iter()
                .filter_map(|id| {
                    origin_candidates
                        .iter()
                        .find(|(p, _)| p.id == *id)
                        .map(|(p, _)| p.name.clone())
                })
                .collect();
            OriginAttribution::Conflicting {
                candidates: names,
                evidence: all_evidence,
            }
        } else {
            OriginAttribution::Unknown {
                reason: "Insufficient evidence".to_string(),
                evidence: if all_evidence.is_empty() {
                    vec!["No matching signals found".to_string()]
                } else {
                    all_evidence
                },
            }
        };

        InfrastructureAttribution {
            target: input.target.to_string(),
            registrar,
            authoritative_dns,
            cdn,
            network,
            application_platform,
            origin,
            rule_version: "2026-11-23.v2",
        }
    }

    /// Project role-separated attribution into legacy `DetectionResult` for
    /// backwards-compatible `HostingObservation` persistence.
    pub(crate) fn detect(&self, input: &DetectionInput) -> DetectionResult {
        let attr = self.attribute(input, None);
        let cdn_detected = !attr.cdn.is_empty();
        let cdn_name = attr.cdn.first().map(|c| c.provider_name.clone());

        if cdn_detected {
            let evidence = match &attr.origin {
                OriginAttribution::Unknown { evidence, .. }
                | OriginAttribution::Observed { evidence, .. }
                | OriginAttribution::Conflicting { evidence, .. } => evidence.clone(),
            };
            return DetectionResult {
                provider_id: None,
                provider_name: None,
                provider_category: None,
                cdn_detected: true,
                cdn_name,
                confidence: Confidence::High,
                detection_method: DetectionMethod::CdnName,
                evidence,
            };
        }

        if let Some(platform) = attr.application_platform.first() {
            return DetectionResult {
                provider_id: platform.provider_id.clone(),
                provider_name: Some(platform.provider_name.clone()),
                provider_category: Some("Edge/PaaS".to_string()),
                cdn_detected: false,
                cdn_name: None,
                confidence: Confidence::High,
                detection_method: DetectionMethod::Cname,
                evidence: platform.evidence.clone(),
            };
        }

        match attr.origin {
            OriginAttribution::Observed {
                provider_id,
                provider_name,
                provider_category,
                status,
                evidence,
            } => {
                let (confidence, detection_method) = match status {
                    AttributionStatus::SupportedAttribution => {
                        (Confidence::High, DetectionMethod::Cname)
                    }
                    _ => (Confidence::Medium, DetectionMethod::Asn),
                };
                DetectionResult {
                    provider_id: Some(provider_id),
                    provider_name: Some(provider_name),
                    provider_category: Some(provider_category),
                    cdn_detected: false,
                    cdn_name: None,
                    confidence,
                    detection_method,
                    evidence,
                }
            }
            OriginAttribution::Conflicting { evidence, .. }
            | OriginAttribution::Unknown { evidence, .. } => DetectionResult {
                provider_id: None,
                provider_name: None,
                provider_category: None,
                cdn_detected: false,
                cdn_name: None,
                confidence: Confidence::Low,
                detection_method: DetectionMethod::Combined,
                evidence,
            },
        }
    }
}

impl Default for HostingDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect hosting for a single target and save to storage.
pub fn detect_hosting(
    target: &str,
    dns: Option<&DnsObservation>,
    http: Option<&HttpObservation>,
    asn_by_ip: &HashMap<String, AsnObservation>,
    storage: &Arc<dyn StorageEngine>,
) -> crate::error::Result<HostingObservation> {
    let detector = HostingDetector::new();

    let input = DetectionInput {
        target,
        dns,
        http,
        asn_by_ip,
    };

    let result = detector.detect(&input);

    let obs = HostingObservation {
        item_type: ItemType::HostingDetection,
        id: Uuid::new_v4().to_string(),
        target: target.to_string(),
        provider_id: result.provider_id,
        provider_name: result.provider_name,
        provider_category: result.provider_category,
        cdn_detected: result.cdn_detected,
        cdn_name: result.cdn_name,
        confidence: result.confidence,
        detection_method: result.detection_method,
        evidence: result.evidence,
        checked_at: Utc::now(),
        source: "detector".to_string(),
    };

    storage.save(&obs)?;
    Ok(obs)
}

/// Detect hosting for multiple targets in batch.
pub fn detect_hosting_batch(
    targets: &[String],
    dns_map: &HashMap<String, DnsObservation>,
    http_map: &HashMap<String, HttpObservation>,
    asn_by_ip: &HashMap<String, AsnObservation>,
    storage: &Arc<dyn StorageEngine>,
) -> crate::error::Result<Vec<HostingObservation>> {
    let detector = HostingDetector::new();
    let mut observations = Vec::new();

    for target in targets {
        let dns = dns_map.get(target);
        let http = http_map.get(target);

        let input = DetectionInput {
            target,
            dns,
            http,
            asn_by_ip,
        };

        let result = detector.detect(&input);

        let obs = HostingObservation {
            item_type: ItemType::HostingDetection,
            id: Uuid::new_v4().to_string(),
            target: target.clone(),
            provider_id: result.provider_id,
            provider_name: result.provider_name,
            provider_category: result.provider_category,
            cdn_detected: result.cdn_detected,
            cdn_name: result.cdn_name,
            confidence: result.confidence,
            detection_method: result.detection_method,
            evidence: result.evidence,
            checked_at: Utc::now(),
            source: "detector".to_string(),
        };

        observations.push(obs);
    }

    if !observations.is_empty() {
        storage.save_batch(&observations)?;
    }

    Ok(observations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemType;
    use chrono::Utc;

    fn make_dns(target: &str, cnames: Vec<&str>, ips: Vec<&str>) -> DnsObservation {
        DnsObservation {
            item_type: ItemType::DnsRecord,
            id: "test".to_string(),
            asset_type: "domain".to_string(),
            target: target.to_string(),
            a: ips.into_iter().map(String::from).collect(),
            aaaa: vec![],
            cname: cnames.into_iter().map(String::from).collect(),
            mx: vec![],
            ns: vec![],
            txt: vec![],
            soa: vec![],
            checked_at: Utc::now(),
            source: "test".to_string(),
        }
    }

    fn make_http(target: &str, cdn_name: Option<&str>, webserver: Option<&str>) -> HttpObservation {
        HttpObservation {
            item_type: ItemType::HttpCheck,
            id: "test".to_string(),
            asset_type: "domain".to_string(),
            target: target.to_string(),
            status: Some(200),
            response_time_ms: 100,
            final_url: None,
            redirect_chain: vec![],
            tls: None,
            tech: vec![],
            webserver: webserver.map(String::from),
            title: None,
            cdn_name: cdn_name.map(String::from),
            favicon_hash: None,
            checked_at: Utc::now(),
            source: "test".to_string(),
            error_kind: None,
            error_message: None,
        }
    }

    fn make_asn(ip: &str, asn: u32, org: &str) -> AsnObservation {
        AsnObservation {
            item_type: ItemType::AsnRecord,
            id: "test".to_string(),
            target: ip.to_string(),
            host: "test.com".to_string(),
            asn,
            org: org.to_string(),
            country: "US".to_string(),
            checked_at: Utc::now(),
            source: "test".to_string(),
        }
    }

    #[test]
    fn test_detect_cloudflare_cdn() {
        let detector = HostingDetector::new();
        let http = make_http("example.com", Some("cloudflare"), None);
        let asn_map = HashMap::new();

        let input = DetectionInput {
            target: "example.com",
            dns: None,
            http: Some(&http),
            asn_by_ip: &asn_map,
        };

        let result = detector.detect(&input);
        assert!(result.cdn_detected);
        assert_eq!(result.cdn_name, Some("Cloudflare".to_string()));
        assert!(
            result.provider_id.is_none(),
            "origin must stay unknown behind CDN"
        );
    }

    #[test]
    fn test_detect_cloudfront_masks_origin() {
        let detector = HostingDetector::new();
        let http = make_http("example.com", Some("cloudfront"), None);
        let asn_map = HashMap::new();

        let input = DetectionInput {
            target: "example.com",
            dns: None,
            http: Some(&http),
            asn_by_ip: &asn_map,
        };

        let attr = detector.attribute(&input, None);
        assert_eq!(attr.cdn.len(), 1);
        assert_eq!(attr.cdn[0].provider_name, "CloudFront");
        assert!(matches!(attr.origin, OriginAttribution::Unknown { .. }));

        let result = detector.detect(&input);
        assert!(result.cdn_detected);
        assert_eq!(result.cdn_name, Some("CloudFront".to_string()));
        assert!(result.provider_id.is_none());
    }

    #[test]
    fn test_iis_and_nginx_headers_do_not_attribute_cloud_provider() {
        let detector = HostingDetector::new();
        let asn_map = HashMap::new();
        for server in ["Microsoft-IIS/10.0", "nginx/1.25", "Apache/2.4"] {
            let http = make_http("example.com", None, Some(server));
            let input = DetectionInput {
                target: "example.com",
                dns: None,
                http: Some(&http),
                asn_by_ip: &asn_map,
            };
            let result = detector.detect(&input);
            assert!(
                result.provider_id.is_none(),
                "generic server '{server}' must not attribute a cloud provider"
            );
        }
    }

    #[test]
    fn test_cloudflare_plus_aws_network_separates_roles() {
        let detector = HostingDetector::new();
        let mut dns = make_dns("example.com", vec![], vec!["1.2.3.4"]);
        dns.ns = vec!["ada.ns.cloudflare.com".to_string()];
        let http = make_http("example.com", Some("cloudflare"), None);
        let mut asn_map = HashMap::new();
        asn_map.insert(
            "1.2.3.4".to_string(),
            make_asn("1.2.3.4", 16509, "AMAZON-02"),
        );

        let input = DetectionInput {
            target: "example.com",
            dns: Some(&dns),
            http: Some(&http),
            asn_by_ip: &asn_map,
        };
        let attr = detector.attribute(&input, None);
        assert!(!attr.authoritative_dns.is_empty());
        assert!(!attr.cdn.is_empty());
        assert_eq!(attr.network.len(), 1);
        assert_eq!(attr.network[0].asn, 16509);
        assert!(matches!(attr.origin, OriginAttribution::Unknown { .. }));
    }

    #[test]
    fn test_detect_aws_from_cname() {
        let detector = HostingDetector::new();
        let dns = make_dns(
            "example.com",
            vec!["example.s3.amazonaws.com"],
            vec!["1.2.3.4"],
        );
        let asn_map = HashMap::new();

        let input = DetectionInput {
            target: "example.com",
            dns: Some(&dns),
            http: None,
            asn_by_ip: &asn_map,
        };

        let result = detector.detect(&input);
        assert_eq!(result.provider_id, Some("aws".to_string()));
        assert_eq!(result.confidence, Confidence::High);
        assert_eq!(result.detection_method, DetectionMethod::Cname);
    }

    #[test]
    fn test_detect_vercel_from_cname() {
        let detector = HostingDetector::new();
        let dns = make_dns("example.com", vec!["cname.vercel-dns.com"], vec!["1.2.3.4"]);
        let asn_map = HashMap::new();

        let input = DetectionInput {
            target: "example.com",
            dns: Some(&dns),
            http: None,
            asn_by_ip: &asn_map,
        };

        let result = detector.detect(&input);
        assert_eq!(result.provider_id, Some("vercel".to_string()));
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_detect_from_asn() {
        let detector = HostingDetector::new();
        let dns = make_dns("example.com", vec![], vec!["1.2.3.4"]);
        let mut asn_map = HashMap::new();
        asn_map.insert(
            "1.2.3.4".to_string(),
            make_asn("1.2.3.4", 13335, "CLOUDFLARENET"),
        );

        let input = DetectionInput {
            target: "example.com",
            dns: Some(&dns),
            http: None,
            asn_by_ip: &asn_map,
        };

        let result = detector.detect(&input);
        assert!(result.cdn_detected);
        assert_eq!(result.cdn_name, Some("Cloudflare".to_string()));
        assert!(result.provider_id.is_none());
    }

    #[test]
    fn test_unknown_when_no_signals() {
        let detector = HostingDetector::new();
        let asn_map = HashMap::new();

        let input = DetectionInput {
            target: "example.com",
            dns: None,
            http: None,
            asn_by_ip: &asn_map,
        };

        let result = detector.detect(&input);
        assert!(result.provider_id.is_none());
        assert_eq!(result.confidence, Confidence::Low);
    }
}
