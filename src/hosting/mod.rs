//! Hosting provider detection module.
//!
//! Provides heuristic-based detection of hosting providers, CDNs, and infrastructure
//! from DNS, HTTP, and ASN signals. Follows the principle: UNKNOWN > wrong.

mod detector;
mod providers;

pub use detector::{
    detect_hosting, detect_hosting_batch, AttributionStatus, DetectionInput, HostingDetector,
    InfrastructureAttribution, NetworkAttribution, OriginAttribution, ProviderAttribution,
    RegistrarAttribution,
};
