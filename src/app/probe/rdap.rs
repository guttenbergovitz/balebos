use crate::error::{ProbeError, Result};
use crate::org::{DomainContact, DomainRegistrationRecord};
use crate::storage::JsonlStorage;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Deserialize, Clone)]
struct RdapEvent {
    #[serde(rename = "eventAction")]
    action: String,
    #[serde(rename = "eventDate")]
    date: String,
}

#[derive(Debug, Deserialize, Clone)]
struct RdapEntity {
    roles: Option<Vec<String>>,
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

pub fn run_rdap_probe(storage: &JsonlStorage, domain: &str) -> Result<DomainRegistrationRecord> {
    let record = run_rdap_probe_no_save(domain)?;
    storage.save(&record)?;
    Ok(record)
}

pub fn run_rdap_probe_no_save(domain: &str) -> Result<DomainRegistrationRecord> {
    match perform_rdap_query(domain) {
        Ok(record) => Ok(record),
        Err(_) => {
            let record = perform_whois_query(domain)?;
            Ok(record)
        }
    }
}

fn perform_rdap_query(domain: &str) -> Result<DomainRegistrationRecord> {
    let output = Command::new("rdap")
        .arg(domain)
        .arg("--json")
        .output()
        .map_err(ProbeError::Execution)?;

    if !output.status.success() {
        return Err(ProbeError::RdapFailed.into());
    }

    let parsed: RdapOutput =
        serde_json::from_slice(&output.stdout).map_err(ProbeError::ParseError)?;

    let mut record = DomainRegistrationRecord::new(domain.to_string(), "rdap".to_string());

    if let Some(events) = parsed.events {
        for event in events {
            let date = DateTime::parse_from_rfc3339(&event.date)
                .map(|dt| dt.with_timezone(&Utc))
                .ok();

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

    Ok(record)
}

fn process_entities(entities: &[RdapEntity], record: &mut DomainRegistrationRecord) {
    for entity in entities {
        if let Some(roles) = &entity.roles {
            let contact = parse_vcard(&entity.vcard);

            if roles.contains(&"registrar".to_string()) {
                if let Some(c) = &contact {
                    record.registrar = c.name.clone().or_else(|| c.organization.clone());
                }
            }
            if roles.contains(&"registrant".to_string()) {
                record.registrant = contact.clone();
            }
            if roles.contains(&"administrative".to_string()) {
                record.admin = contact.clone();
            }
            if roles.contains(&"technical".to_string()) {
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
        let tag = prop_arr.get(0)?.as_str()?;
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

fn perform_whois_query(domain: &str) -> Result<DomainRegistrationRecord> {
    let output = Command::new("whois")
        .arg(domain)
        .output()
        .map_err(ProbeError::Execution)?;

    let text = String::from_utf8_lossy(&output.stdout);
    let mut record = DomainRegistrationRecord::new(domain.to_string(), "whois".to_string());

    for line in text.lines() {
        let line = line.trim();
        let lower = line.to_lowercase();
        if lower.starts_with("registrar:") {
            record.registrar = line.split(':').nth(1).map(|s| s.trim().to_string());
        } else if lower.starts_with("creation date:") || lower.starts_with("registered on:") {
            record.registered_at = parse_whois_date(line);
        } else if lower.starts_with("registry expiry date:") || lower.starts_with("expiry date:") {
            record.expires_at = parse_whois_date(line);
        } else if lower.starts_with("updated date:") || lower.starts_with("last updated:") {
            record.updated_at = parse_whois_date(line);
        }
        // Basic registrant info from WHOIS if available
        else if lower.starts_with("registrant name:") {
            let mut c = record.registrant.take().unwrap_or(DomainContact {
                name: None,
                organization: None,
                email: None,
            });
            c.name = line.split(':').nth(1).map(|s| s.trim().to_string());
            record.registrant = Some(c);
        } else if lower.starts_with("registrant email:") {
            let mut c = record.registrant.take().unwrap_or(DomainContact {
                name: None,
                organization: None,
                email: None,
            });
            c.email = line.split(':').nth(1).map(|s| s.trim().to_string());
            record.registrant = Some(c);
        }
    }

    Ok(record)
}

fn parse_whois_date(line: &str) -> Option<DateTime<Utc>> {
    let date_str = line.split(':').nth(1)?.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
        return Some(dt.with_timezone(&Utc));
    }
    None
}
