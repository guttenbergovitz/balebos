//! Hosting provider detection algorithm.
//!
//! Detection priority (strict order):
//! 1. CDN Detection (highest priority) - Match cdn_name from httpx
//! 2. CNAME Pattern Match (strong signal) - Match CNAME against provider patterns
//! 3. ASN Match (fallback) - Match IP's ASN against provider ASN list
//!
//! Conflict resolution: Multiple providers detected -> UNKNOWN

use super::providers::{Provider, ProviderKnowledgeBase};
use crate::app::probe::{
    AsnObservation, Confidence, DetectionMethod, DnsObservation, HostingObservation,
    HttpObservation,
};
use crate::inventory::ItemType;
use crate::storage::{StorageEngine, StorageEngineExt};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Hosting detection engine.
pub struct HostingDetector {
    kb: ProviderKnowledgeBase,
}

/// Input signals for hosting detection.
pub struct DetectionInput<'a> {
    #[allow(dead_code)]
    pub target: &'a str,
    pub dns: Option<&'a DnsObservation>,
    pub http: Option<&'a HttpObservation>,
    pub asn_by_ip: &'a HashMap<String, AsnObservation>,
}

/// Intermediate detection result before being stored.
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

    /// Detect hosting provider from available signals.
    pub fn detect(&self, input: &DetectionInput) -> DetectionResult {
        // Step 1: Check CDN name (highest priority)
        if let Some(cdn_result) = self.detect_from_cdn(input) {
            return cdn_result;
        }

        // Step 2: Check CNAME patterns (strong signal)
        if let Some(cname_result) = self.detect_from_cname(input) {
            return cname_result;
        }

        // Step 3: Check ASN (fallback)
        if let Some(asn_result) = self.detect_from_asn(input) {
            return asn_result;
        }

        // Step 4: Check webserver header (weak signal)
        if let Some(header_result) = self.detect_from_header(input) {
            return header_result;
        }

        // No detection possible
        DetectionResult {
            provider_id: None,
            provider_name: None,
            provider_category: None,
            cdn_detected: false,
            cdn_name: None,
            confidence: Confidence::Low,
            detection_method: DetectionMethod::Combined,
            evidence: vec!["No matching signals found".to_string()],
        }
    }

    /// Detect from CDN name (httpx cdn_name field).
    fn detect_from_cdn(&self, input: &DetectionInput) -> Option<DetectionResult> {
        let http = input.http?;
        let cdn_name = http.cdn_name.as_ref()?;

        let matching_providers = self.kb.find_by_cdn_name(cdn_name);

        if matching_providers.is_empty() {
            // CDN detected but not in our knowledge base
            return Some(DetectionResult {
                provider_id: None,
                provider_name: None,
                provider_category: None,
                cdn_detected: true,
                cdn_name: Some(cdn_name.clone()),
                confidence: Confidence::High,
                detection_method: DetectionMethod::CdnName,
                evidence: vec![format!("CDN detected: {} (not in KB)", cdn_name)],
            });
        }

        if matching_providers.len() == 1 {
            let provider = &matching_providers[0];

            // Special case: CloudFront -> CDN = CloudFront, Hosting = AWS
            if provider.id == "aws" && cdn_name.to_lowercase().contains("cloudfront") {
                return Some(DetectionResult {
                    provider_id: Some("aws".to_string()),
                    provider_name: Some("Amazon Web Services".to_string()),
                    provider_category: Some("Hyperscaler".to_string()),
                    cdn_detected: true,
                    cdn_name: Some("CloudFront".to_string()),
                    confidence: Confidence::Confirmed,
                    detection_method: DetectionMethod::CdnName,
                    evidence: vec![format!("CDN {} maps to AWS CloudFront", cdn_name)],
                });
            }

            // For CDN providers, hosting is UNKNOWN (CDN masks origin)
            if provider.is_cdn() {
                return Some(DetectionResult {
                    provider_id: None, // Hosting is unknown when behind CDN
                    provider_name: None,
                    provider_category: None,
                    cdn_detected: true,
                    cdn_name: Some(provider.name.clone()),
                    confidence: Confidence::Confirmed,
                    detection_method: DetectionMethod::CdnName,
                    evidence: vec![format!("Behind CDN: {}", provider.name)],
                });
            }

            // Non-CDN provider matched by CDN name (e.g., Vercel, Netlify)
            return Some(DetectionResult {
                provider_id: Some(provider.id.clone()),
                provider_name: Some(provider.name.clone()),
                provider_category: Some(provider.category.to_string()),
                cdn_detected: false,
                cdn_name: None,
                confidence: Confidence::Confirmed,
                detection_method: DetectionMethod::CdnName,
                evidence: vec![format!("CDN name {} matches {}", cdn_name, provider.name)],
            });
        }

        // Multiple providers matched - conflict
        let names: Vec<_> = matching_providers.iter().map(|p| p.name.as_str()).collect();
        Some(DetectionResult {
            provider_id: None,
            provider_name: None,
            provider_category: None,
            cdn_detected: true,
            cdn_name: Some(cdn_name.clone()),
            confidence: Confidence::Low,
            detection_method: DetectionMethod::CdnName,
            evidence: vec![format!(
                "CDN {} matched multiple providers: {}",
                cdn_name,
                names.join(", ")
            )],
        })
    }

    /// Detect from CNAME patterns.
    fn detect_from_cname(&self, input: &DetectionInput) -> Option<DetectionResult> {
        let dns = input.dns?;

        if dns.cname.is_empty() {
            return None;
        }

        let mut all_matches: Vec<&Provider> = Vec::new();
        let mut evidence = Vec::new();

        for cname in &dns.cname {
            let matches = self.kb.find_by_cname(cname);
            for provider in matches {
                evidence.push(format!("CNAME {} matches {}", cname, provider.name));
                if !all_matches.iter().any(|p| p.id == provider.id) {
                    all_matches.push(provider);
                }
            }
        }

        if all_matches.is_empty() {
            return None;
        }

        if all_matches.len() == 1 {
            let provider = all_matches[0];
            return Some(DetectionResult {
                provider_id: Some(provider.id.clone()),
                provider_name: Some(provider.name.clone()),
                provider_category: Some(provider.category.to_string()),
                cdn_detected: provider.is_cdn(),
                cdn_name: if provider.is_cdn() {
                    Some(provider.name.clone())
                } else {
                    None
                },
                confidence: Confidence::High,
                detection_method: DetectionMethod::Cname,
                evidence,
            });
        }

        // Multiple providers - check if they're all from same parent (e.g., AWS services)
        let unique_ids: Vec<_> = all_matches.iter().map(|p| &p.id).collect();
        let names: Vec<_> = all_matches.iter().map(|p| p.name.as_str()).collect();

        // If all match the same provider ID, use it
        if unique_ids.iter().all(|id| *id == unique_ids[0]) {
            let provider = all_matches[0];
            return Some(DetectionResult {
                provider_id: Some(provider.id.clone()),
                provider_name: Some(provider.name.clone()),
                provider_category: Some(provider.category.to_string()),
                cdn_detected: provider.is_cdn(),
                cdn_name: if provider.is_cdn() {
                    Some(provider.name.clone())
                } else {
                    None
                },
                confidence: Confidence::High,
                detection_method: DetectionMethod::Cname,
                evidence,
            });
        }

        // Actual conflict
        Some(DetectionResult {
            provider_id: None,
            provider_name: None,
            provider_category: None,
            cdn_detected: false,
            cdn_name: None,
            confidence: Confidence::Low,
            detection_method: DetectionMethod::Cname,
            evidence: vec![format!(
                "CNAME patterns matched multiple providers: {}",
                names.join(", ")
            )],
        })
    }

    /// Detect from ASN.
    fn detect_from_asn(&self, input: &DetectionInput) -> Option<DetectionResult> {
        let dns = input.dns?;

        if dns.a.is_empty() && dns.aaaa.is_empty() {
            return None;
        }

        let mut all_matches: Vec<&Provider> = Vec::new();
        let mut evidence = Vec::new();

        // Check all IPs
        for ip in dns.a.iter().chain(dns.aaaa.iter()) {
            if let Some(asn_obs) = input.asn_by_ip.get(ip) {
                if asn_obs.asn == 0 {
                    continue;
                }

                let matches = self.kb.find_by_asn(asn_obs.asn);
                for provider in &matches {
                    evidence.push(format!(
                        "IP {} ASN {} ({}) matches {}",
                        ip, asn_obs.asn, asn_obs.org, provider.name
                    ));
                    if !all_matches.iter().any(|p| p.id == provider.id) {
                        all_matches.push(provider);
                    }
                }

                // Even if no provider match, record ASN info as evidence
                if matches.is_empty() {
                    evidence.push(format!(
                        "IP {} ASN {} ({}) - no provider match",
                        ip, asn_obs.asn, asn_obs.org
                    ));
                }
            }
        }

        if all_matches.is_empty() {
            if !evidence.is_empty() {
                // We have ASN info but no provider match
                return Some(DetectionResult {
                    provider_id: None,
                    provider_name: None,
                    provider_category: None,
                    cdn_detected: false,
                    cdn_name: None,
                    confidence: Confidence::Low,
                    detection_method: DetectionMethod::Asn,
                    evidence,
                });
            }
            return None;
        }

        if all_matches.len() == 1 {
            let provider = all_matches[0];
            return Some(DetectionResult {
                provider_id: Some(provider.id.clone()),
                provider_name: Some(provider.name.clone()),
                provider_category: Some(provider.category.to_string()),
                cdn_detected: provider.is_cdn(),
                cdn_name: if provider.is_cdn() {
                    Some(provider.name.clone())
                } else {
                    None
                },
                confidence: Confidence::Medium,
                detection_method: DetectionMethod::Asn,
                evidence,
            });
        }

        // Multiple providers
        let names: Vec<_> = all_matches.iter().map(|p| p.name.as_str()).collect();
        Some(DetectionResult {
            provider_id: None,
            provider_name: None,
            provider_category: None,
            cdn_detected: false,
            cdn_name: None,
            confidence: Confidence::Low,
            detection_method: DetectionMethod::Asn,
            evidence: vec![format!(
                "ASN matched multiple providers: {}",
                names.join(", ")
            )],
        })
    }

    /// Detect from webserver header (weak signal).
    fn detect_from_header(&self, input: &DetectionInput) -> Option<DetectionResult> {
        let http = input.http?;
        let webserver = http.webserver.as_ref()?;

        let matches = self.kb.find_by_header(webserver);

        if matches.is_empty() {
            return None;
        }

        if matches.len() == 1 {
            let provider = &matches[0];
            return Some(DetectionResult {
                provider_id: Some(provider.id.clone()),
                provider_name: Some(provider.name.clone()),
                provider_category: Some(provider.category.to_string()),
                cdn_detected: provider.is_cdn(),
                cdn_name: if provider.is_cdn() {
                    Some(provider.name.clone())
                } else {
                    None
                },
                confidence: Confidence::Low, // Headers are weak signals
                detection_method: DetectionMethod::Header,
                evidence: vec![format!(
                    "Server header '{}' matches {}",
                    webserver, provider.name
                )],
            });
        }

        // Multiple matches from header - low confidence
        let names: Vec<_> = matches.iter().map(|p| p.name.as_str()).collect();
        Some(DetectionResult {
            provider_id: None,
            provider_name: None,
            provider_category: None,
            cdn_detected: false,
            cdn_name: None,
            confidence: Confidence::Low,
            detection_method: DetectionMethod::Header,
            evidence: vec![format!(
                "Header '{}' matched multiple providers: {}",
                webserver,
                names.join(", ")
            )],
        })
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
        assert_eq!(result.confidence, Confidence::Confirmed);
        // Hosting should be unknown (behind CDN)
        assert!(result.provider_id.is_none());
    }

    #[test]
    fn test_detect_cloudfront_aws() {
        let detector = HostingDetector::new();
        let http = make_http("example.com", Some("cloudfront"), None);
        let asn_map = HashMap::new();

        let input = DetectionInput {
            target: "example.com",
            dns: None,
            http: Some(&http),
            asn_by_ip: &asn_map,
        };

        let result = detector.detect(&input);
        assert!(result.cdn_detected);
        assert_eq!(result.cdn_name, Some("CloudFront".to_string()));
        assert_eq!(result.provider_id, Some("aws".to_string()));
        assert_eq!(result.confidence, Confidence::Confirmed);
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
        assert_eq!(result.provider_id, Some("cloudflare".to_string()));
        assert_eq!(result.confidence, Confidence::Medium);
        assert_eq!(result.detection_method, DetectionMethod::Asn);
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
