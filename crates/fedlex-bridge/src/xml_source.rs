//! Abstraktion über den XML-Download (Manifestations-URL → Rohtext).
//!
//! Analog zum [`SparqlClient`]-Trait der jolux-Crate hält dieses Trait die
//! Fetcher-Logik transportfrei und deterministisch testbar.
//!
//! [`SparqlClient`]: fedlex_jolux::SparqlClient

use crate::error::BridgeError;
use crate::timeouts::HttpTimeouts;
use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Quelle für AKN-XML-Manifestationen.
#[async_trait]
pub trait XmlSource: Send + Sync {
    /// Lädt das XML hinter der gegebenen Manifestations-URL.
    async fn fetch(&self, url: &str) -> Result<String, BridgeError>;
}

/// Default-Obergrenze eines XML-Downloads (67 §H-6).
///
/// Konsolidierte Erlasse sind 1–10 MB; 32 MB sind großzügig, aber endlich —
/// ohne Grenze lädt `fetch` beliebig große Bodies in einen `String`.
pub const DEFAULT_MAX_XML_BYTES: u64 = 32 * 1024 * 1024;

/// Produktions-Quelle über HTTP (`fedlex.data.admin.ch/filestore/...`).
#[derive(Debug, Clone)]
pub struct HttpXmlSource {
    http: reqwest::Client,
    max_bytes: u64,
}

impl HttpXmlSource {
    /// Neue HTTP-Quelle mit Default-Timeouts (H-1: jeder Client dieser
    /// Crate trägt zwingend Zeitgrenzen — deshalb kein `Default` mehr).
    pub fn new() -> Result<Self, BridgeError> {
        Self::with_timeouts(HttpTimeouts::default())
    }

    /// Neue HTTP-Quelle mit expliziten Zeitgrenzen.
    pub fn with_timeouts(timeouts: HttpTimeouts) -> Result<Self, BridgeError> {
        Ok(Self {
            http: timeouts.client()?,
            max_bytes: DEFAULT_MAX_XML_BYTES,
        })
    }

    /// Setzt die Download-Obergrenze (Default [`DEFAULT_MAX_XML_BYTES`]).
    pub fn with_max_bytes(mut self, max_bytes: u64) -> Self {
        self.max_bytes = max_bytes;
        self
    }
}

#[async_trait]
impl XmlSource for HttpXmlSource {
    async fn fetch(&self, url: &str) -> Result<String, BridgeError> {
        use futures_util::StreamExt;

        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| BridgeError::Download(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(BridgeError::Download(format!(
                "HTTP {} für {url}",
                resp.status()
            )));
        }

        // Größen-Guard (67 §H-6): erst der angekündigte Content-Length,
        // dann die tatsächliche Stream-Summe — beides gegen `max_bytes`.
        if let Some(len) = resp.content_length()
            && len > self.max_bytes
        {
            return Err(BridgeError::Download(format!(
                "XML zu gross: {len} Bytes angekuendigt, Limit {} (MCP_XML_MAX_BYTES)",
                self.max_bytes
            )));
        }

        let mut buf: Vec<u8> = Vec::new();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| BridgeError::Download(e.to_string()))?;
            if (buf.len() + chunk.len()) as u64 > self.max_bytes {
                return Err(BridgeError::Download(format!(
                    "XML zu gross: ueber {} Bytes empfangen, Limit {} (MCP_XML_MAX_BYTES)",
                    buf.len() + chunk.len(),
                    self.max_bytes
                )));
            }
            buf.extend_from_slice(&chunk);
        }

        String::from_utf8(buf)
            .map_err(|e| BridgeError::Download(format!("XML ist kein UTF-8: {e}")))
    }
}

/// Deterministische Mock-Quelle für Tests. Liefert für jede URL dasselbe
/// vorbereitete XML und zählt die Abrufe (für Cache-Assertions).
#[derive(Debug, Clone)]
pub struct MockXmlSource {
    xml: String,
    fetches: Arc<AtomicUsize>,
}

impl MockXmlSource {
    /// Mock-Quelle mit dem gegebenen XML-Inhalt.
    pub fn new(xml: impl Into<String>) -> Self {
        Self {
            xml: xml.into(),
            fetches: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Wie oft `fetch` aufgerufen wurde.
    pub fn fetch_count(&self) -> usize {
        self.fetches.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl XmlSource for MockXmlSource {
    async fn fetch(&self, _url: &str) -> Result<String, BridgeError> {
        self.fetches.fetch_add(1, Ordering::SeqCst);
        Ok(self.xml.clone())
    }
}
