//! Produktions-Implementierung des [`SparqlClient`]-Traits über HTTP.
//!
//! Die jolux-Primitive sind transportfrei — dieser Client liefert den
//! Live-Transport gegen `fedlex.data.admin.ch` (oder einen Spiegel).

use crate::error::BridgeError;
use crate::timeouts::HttpTimeouts;
use async_trait::async_trait;
use fedlex_jolux::{JoluxError, SparqlClient, SparqlResults};

/// Standard-Endpoint des öffentlichen Fedlex-Triplestores.
pub const FEDLEX_ENDPOINT: &str = "https://fedlex.data.admin.ch/sparqlendpoint";

/// HTTP-SPARQL-Client (POST `application/x-www-form-urlencoded`,
/// Accept `application/sparql-results+json`).
///
/// Achtung Falltrap (Live-Befund 2026-06-10, JOLux-Lexikon). Die Fedlex-WAF
/// blockt bestimmte Query-Formen (`SELECT DISTINCT` plus
/// `citationFromLegalResource` plus URL-Literal ergibt HTTP 400). Die
/// jolux-Primitive sind entsprechend formuliert. Eigene Queries durch diesen
/// Client müssen das ebenfalls beachten.
#[derive(Debug, Clone)]
pub struct HttpSparqlClient {
    http: reqwest::Client,
    endpoint: String,
}

impl HttpSparqlClient {
    /// Client gegen den gegebenen Endpoint mit Default-Timeouts (H-1:
    /// jeder Client dieser Crate trägt zwingend Zeitgrenzen).
    pub fn new(endpoint: impl Into<String>) -> Result<Self, BridgeError> {
        Self::with_timeouts(endpoint, HttpTimeouts::default())
    }

    /// Client gegen den gegebenen Endpoint mit expliziten Zeitgrenzen.
    pub fn with_timeouts(
        endpoint: impl Into<String>,
        timeouts: HttpTimeouts,
    ) -> Result<Self, BridgeError> {
        Ok(Self {
            http: timeouts.client()?,
            endpoint: endpoint.into(),
        })
    }

    /// Client gegen den öffentlichen Fedlex-Endpoint mit Default-Timeouts.
    pub fn fedlex() -> Result<Self, BridgeError> {
        Self::new(FEDLEX_ENDPOINT)
    }

    /// Client gegen den öffentlichen Fedlex-Endpoint mit expliziten Zeitgrenzen.
    pub fn fedlex_with(timeouts: HttpTimeouts) -> Result<Self, BridgeError> {
        Self::with_timeouts(FEDLEX_ENDPOINT, timeouts)
    }
}

#[async_trait]
impl SparqlClient for HttpSparqlClient {
    async fn query(&self, sparql: &str) -> Result<SparqlResults, JoluxError> {
        let resp = self
            .http
            .post(&self.endpoint)
            .header("Accept", "application/sparql-results+json")
            .form(&[("query", sparql)])
            .send()
            .await
            .map_err(|e| JoluxError::Transport(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(JoluxError::Transport(format!(
                "HTTP {} vom Endpoint",
                resp.status()
            )));
        }
        let body = resp
            .text()
            .await
            .map_err(|e| JoluxError::Transport(e.to_string()))?;
        SparqlResults::from_json(&body)
    }
}
