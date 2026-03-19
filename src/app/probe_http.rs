use crate::storage::JsonlStorage;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::io;
use std::time::{Duration, Instant};

// Struktura reprezentująca obserwację HTTP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpObservation {
    // Typ obserwacji
    pub item_type: String,

    // Relacja do zasobu
    pub asset_type: String,
    pub target: String,

    // Dane obserwacji
    pub status: u16,
    pub response_time_ms: u64,

    // Metadane
    pub checked_at: chrono::DateTime<Utc>,
    pub source: String,
}

// Przeprowadź probe HTTP i zapisz wynik jako obserwację
pub fn http_check(domain: &str, storage: &JsonlStorage) -> Result<HttpObservation, io::Error> {
    // Usuń prefiks protokołu jeśli został podany
    let clean_domain = domain
        .trim()
        .replace("http://", "")
        .replace("https://", "")
        .replace("www.", "");

    // Najpierw próbujemy HTTPS
    let https_url = format!("https://{}", clean_domain);
    let http_url = format!("http://{}", clean_domain);

    // Konfiguracja klienta z timeout
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    // Mierzenie czasu odpowiedzi
    let start = Instant::now();

    // Próba HTTPS, fallback na HTTP
    let response = match client.get(&https_url).send() {
        Ok(resp) => resp,
        Err(_) => {
            // HTTPS nie zadziałał, próbujemy HTTP
            println!("HTTPS failed, falling back to HTTP for {}", domain);
            match client.get(&http_url).send() {
                Ok(resp) => resp,
                Err(e) => return Err(io::Error::new(io::ErrorKind::Other, e.to_string())),
            }
        }
    };

    let duration = start.elapsed().as_millis() as u64;
    let status = response.status().as_u16();

    // Tworzenie obserwacji
    let observation = HttpObservation {
        item_type: "http_check".to_string(),
        asset_type: "domain".to_string(),
        target: clean_domain.to_string(),
        status,
        response_time_ms: duration,
        checked_at: Utc::now(),
        source: "probe_http".to_string(),
    };

    // Zapisanie obserwacji
    storage.save(&observation)?;

    // Komunikat pojawi się tylko raz, w main.rs

    Ok(observation)
}
