use super::process;
use crate::domain::normalize::registrable_domain;
use crate::error::{ProbeError, Result};
use crate::org::{DomainContact, DomainRegistrationRecord};
use crate::storage::{StorageEngine, StorageEngineExt};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde::Deserialize;
use std::process::Command;
use std::sync::Arc;

#[derive(Debug, Deserialize, Clone)]
struct RdapEvent {
    #[serde(rename = "eventAction")]
    action: String,
    #[serde(rename = "eventDate")]
    date: String,
}

#[derive(Debug, Deserialize, Clone)]
struct RdapPublicId {
    #[serde(rename = "type")]
    id_type: Option<String>,
    identifier: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct RdapEntity {
    handle: Option<String>,
    roles: Option<Vec<String>>,
    #[serde(rename = "publicIds")]
    public_ids: Option<Vec<RdapPublicId>>,
    #[serde(rename = "vcardArray")]
    vcard: Option<serde_json::Value>,
    entities: Option<Vec<RdapEntity>>, // Nested entities
}

#[derive(Debug, Deserialize, Clone)]
struct RdapOutput {
    events: Option<Vec<RdapEvent>>,
    entities: Option<Vec<RdapEntity>>,
    status: Option<Vec<String>>,
}

pub fn run_rdap_probe(
    storage: &Arc<dyn StorageEngine>,
    domain: &str,
) -> Result<DomainRegistrationRecord> {
    let record = run_rdap_probe_no_save(domain)?;
    storage.save(&record)?;
    Ok(record)
}

pub fn run_rdap_probe_no_save(domain: &str) -> Result<DomainRegistrationRecord> {
    let lookup_domain = registrable_domain(domain);
    match perform_rdap_query(&lookup_domain) {
        Ok(record) => Ok(record),
        Err(_) => {
            let record = perform_whois_query(&lookup_domain)?;
            Ok(record)
        }
    }
}

/// Parse an authoritative RDAP JSON payload into a `DomainRegistrationRecord`.
pub fn parse_rdap_json(domain: &str, bytes: &[u8]) -> Result<DomainRegistrationRecord> {
    let parsed: RdapOutput = serde_json::from_slice(bytes).map_err(ProbeError::ParseError)?;

    let mut record = DomainRegistrationRecord::new(domain.to_string(), "rdap".to_string());

    if let Some(events) = parsed.events {
        for event in events {
            let date = DateTime::parse_from_rfc3339(&event.date)
                .map(|dt| dt.with_timezone(&Utc))
                .ok();
            if date.is_none() {
                record
                    .raw_dates
                    .push((event.action.clone(), event.date.clone()));
            }

            match event.action.as_str() {
                "registration" => record.registered_at = date,
                "expiration" => record.expires_at = date,
                "last changed" => record.updated_at = date,
                _ => {}
            }
        }
    }

    if let Some(entities) = parsed.entities {
        process_entities(&entities, &mut record);
    }

    if let Some(status) = parsed.status {
        record.status = status;
    }

    record.completeness = Some(if record.registrar.is_some() {
        "complete".to_string()
    } else {
        "missing_registrar".to_string()
    });

    Ok(record)
}

fn perform_rdap_query(domain: &str) -> Result<DomainRegistrationRecord> {
    let output = process::run_bounded(
        Command::new("rdap").arg(domain).arg("--json"),
        std::time::Duration::from_secs(60),
        process::DEFAULT_MAX_OUTPUT,
    )?;

    if !output.status.success() {
        return Err(ProbeError::RdapFailed.into());
    }

    parse_rdap_json(domain, &output.stdout)
}

fn process_entities(entities: &[RdapEntity], record: &mut DomainRegistrationRecord) {
    for entity in entities {
        if let Some(roles) = &entity.roles {
            let contact = parse_vcard(&entity.vcard);

            if roles.iter().any(|r| r == "registrar") {
                if record.registrar.is_none() {
                    if let Some(c) = &contact {
                        record.registrar = c
                            .name
                            .clone()
                            .or_else(|| c.organization.clone())
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty());
                    }
                }
                if record.registrar_handle.is_none() {
                    record.registrar_handle = entity.handle.clone();
                }
                if record.iana_registrar_id.is_none() {
                    if let Some(pids) = &entity.public_ids {
                        for pid in pids {
                            let is_iana = pid
                                .id_type
                                .as_deref()
                                .is_some_and(|t| t.eq_ignore_ascii_case("IANA Registrar ID"));
                            if is_iana {
                                if let Some(id) = &pid.identifier {
                                    record.iana_registrar_id = Some(id.trim().to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            if roles.iter().any(|r| r == "registrant") && record.registrant.is_none() {
                record.registrant = contact.clone();
            }
            if roles.iter().any(|r| r == "administrative") && record.admin.is_none() {
                record.admin = contact.clone();
            }
            if roles.iter().any(|r| r == "technical") && record.tech.is_none() {
                record.tech = contact.clone();
            }
        }

        // Recursively process nested entities
        if let Some(nested) = &entity.entities {
            process_entities(nested, record);
        }
    }
}

fn parse_vcard(vcard: &Option<serde_json::Value>) -> Option<DomainContact> {
    let vcard = vcard.as_ref()?;
    let arr = vcard.as_array()?;
    if arr.len() < 2 {
        return None;
    }

    let properties = arr[1].as_array()?;
    let mut contact = DomainContact {
        name: None,
        organization: None,
        email: None,
    };

    for prop in properties {
        let prop_arr = prop.as_array()?;
        let tag = prop_arr.first()?.as_str()?;
        let value = prop_arr.get(3)?.as_str()?;

        match tag {
            "fn" => contact.name = Some(value.to_string()),
            "org" => contact.organization = Some(value.to_string()),
            "email" => contact.email = Some(value.to_string()),
            _ => {}
        }
    }

    if contact.name.is_none() && contact.organization.is_none() && contact.email.is_none() {
        None
    } else {
        Some(contact)
    }
}

/// Parse raw WHOIS text into a `DomainRegistrationRecord`, preserving
/// unparsed date strings and using `split_once(':')` for RFC3339 timestamps.
pub fn parse_whois_text(domain: &str, text: &str) -> DomainRegistrationRecord {
    let mut record = DomainRegistrationRecord::new(domain.to_string(), "whois".to_string());

    for line in text.lines() {
        let line = line.trim();
        let lower = line.to_lowercase();
        if lower.starts_with("registrar:") {
            if let Some((_, val)) = line.split_once(':') {
                let trimmed = val.trim();
                if !trimmed.is_empty() {
                    record.registrar = Some(trimmed.to_string());
                }
            }
        } else if lower.starts_with("registrar iana id:") {
            if let Some((_, val)) = line.split_once(':') {
                let trimmed = val.trim();
                if !trimmed.is_empty() {
                    record.iana_registrar_id = Some(trimmed.to_string());
                }
            }
        } else if lower.starts_with("creation date:") || lower.starts_with("registered on:") {
            match parse_whois_date(line) {
                Some(dt) => record.registered_at = Some(dt),
                None => {
                    if let Some((_, raw)) = line.split_once(':') {
                        record
                            .raw_dates
                            .push(("registration".to_string(), raw.trim().to_string()));
                    }
                }
            }
        } else if lower.starts_with("registry expiry date:") || lower.starts_with("expiry date:") {
            match parse_whois_date(line) {
                Some(dt) => record.expires_at = Some(dt),
                None => {
                    if let Some((_, raw)) = line.split_once(':') {
                        record
                            .raw_dates
                            .push(("expiration".to_string(), raw.trim().to_string()));
                    }
                }
            }
        } else if lower.starts_with("updated date:") || lower.starts_with("last updated:") {
            match parse_whois_date(line) {
                Some(dt) => record.updated_at = Some(dt),
                None => {
                    if let Some((_, raw)) = line.split_once(':') {
                        record
                            .raw_dates
                            .push(("last changed".to_string(), raw.trim().to_string()));
                    }
                }
            }
        } else if lower.starts_with("registrant name:") {
            if let Some((_, val)) = line.split_once(':') {
                let mut c = record.registrant.take().unwrap_or(DomainContact {
                    name: None,
                    organization: None,
                    email: None,
                });
                c.name = Some(val.trim().to_string());
                record.registrant = Some(c);
            }
        } else if lower.starts_with("registrant email:") {
            if let Some((_, val)) = line.split_once(':') {
                let mut c = record.registrant.take().unwrap_or(DomainContact {
                    name: None,
                    organization: None,
                    email: None,
                });
                c.email = Some(val.trim().to_string());
                record.registrant = Some(c);
            }
        }
    }

    record.completeness = Some(if record.registrar.is_some() {
        "whois_fallback".to_string()
    } else {
        "whois_missing_registrar".to_string()
    });
    record
}

fn perform_whois_query(domain: &str) -> Result<DomainRegistrationRecord> {
    let output = process::run_bounded(
        Command::new("whois").arg(domain),
        std::time::Duration::from_secs(30),
        process::DEFAULT_MAX_OUTPUT,
    )?;

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(parse_whois_text(domain, &text))
}

fn parse_whois_date(line: &str) -> Option<DateTime<Utc>> {
    let (_, date_str) = line.split_once(':')?;
    let date_str = date_str.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
        return Some(dt.with_timezone(&Utc));
    }
    for fmt in [
        "%Y-%m-%dT%H:%M:%SZ",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M:%S %Z",
    ] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(date_str, fmt) {
            return Some(DateTime::from_naive_utc_and_offset(naive, Utc));
        }
    }
    if let Ok(d) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        return d
            .and_hms_opt(0, 0, 0)
            .map(|ndt| DateTime::from_naive_utc_and_offset(ndt, Utc));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registrable_domain_handles_subdomains_and_two_part_tlds() {
        assert_eq!(registrable_domain("www.example.com"), "example.com");
        assert_eq!(registrable_domain("shop.example.co.uk"), "example.co.uk");
        assert_eq!(registrable_domain("example.co.uk"), "example.co.uk");
    }

    #[test]
    fn rdap_parser_extracts_nested_registrar_and_iana_id() {
        let json = br#"{
            "events": [
                {"eventAction": "registration", "eventDate": "2020-01-15T12:34:56Z"},
                {"eventAction": "expiration", "eventDate": "2028-01-15T12:34:56Z"}
            ],
            "entities": [
                {
                    "roles": ["registrant"],
                    "entities": [
                        {
                            "roles": ["registrar"],
                            "handle": "reg-123",
                            "publicIds": [{"type": "IANA Registrar ID", "identifier": "146"}],
                            "vcardArray": ["vcard", [["fn", {}, "text", "GoDaddy.com, LLC"]]]
                        }
                    ]
                }
            ],
            "status": ["client transfer prohibited"]
        }"#;

        let rec = parse_rdap_json("example.com", json).unwrap();
        assert_eq!(rec.registrar.as_deref(), Some("GoDaddy.com, LLC"));
        assert_eq!(rec.iana_registrar_id.as_deref(), Some("146"));
        assert_eq!(rec.registrar_handle.as_deref(), Some("reg-123"));
        assert_eq!(rec.completeness.as_deref(), Some("complete"));
        assert!(rec.registered_at.is_some());
        assert!(rec.expires_at.is_some());
    }

    #[test]
    fn rdap_parser_flags_missing_registrar_completeness() {
        let json = br#"{"events": [], "entities": [], "status": ["active"]}"#;
        let rec = parse_rdap_json("example.uk", json).unwrap();
        assert!(rec.registrar.is_none());
        assert_eq!(rec.completeness.as_deref(), Some("missing_registrar"));
    }

    #[test]
    fn whois_parser_handles_rfc3339_colons_and_iana_id() {
        let whois = "\
Registrar: Example Registrar Ltd
Registrar IANA ID: 9999
Creation Date: 2020-01-15T12:34:56Z
Registry Expiry Date: 2028-01-15 00:00:00
Updated Date: unparseable-date-string
";
        let rec = parse_whois_text("example.com", whois);
        assert_eq!(rec.registrar.as_deref(), Some("Example Registrar Ltd"));
        assert_eq!(rec.iana_registrar_id.as_deref(), Some("9999"));
        assert_eq!(
            rec.registered_at
                .unwrap()
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
            "2020-01-15 12:34:56"
        );
        assert!(rec.expires_at.is_some());
        assert!(rec.updated_at.is_none());
        assert_eq!(
            rec.raw_dates,
            vec![(
                "last changed".to_string(),
                "unparseable-date-string".to_string()
            )]
        );
    }
}
