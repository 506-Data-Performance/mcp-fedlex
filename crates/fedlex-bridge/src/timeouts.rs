//! Zeitgrenzen für alle ausgehenden HTTP-Aufrufe der Bridge.
//!
//! Ohne Grenzen bindet ein langsamer Upstream (Fedlex-WAF, Filestore) Tasks
//! unbegrenzt an offenen Sockets, bis der Pod kippt — deshalb trägt jeder
//! Client dieser Crate zwingend ein Connect- und ein Gesamt-Timeout
//! (67_HARDENING_AND_SOTA_ROADMAP §H-1). Die Werte sind bewusst großzügig
//! (Fedlex-SPARQL braucht für komplexe Queries mehrere Sekunden), aber endlich.

use crate::error::BridgeError;
use std::time::Duration;

/// Connect- und Gesamt-Timeout eines HTTP-Clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpTimeouts {
    /// Zeitgrenze für den TCP/TLS-Verbindungsaufbau.
    pub connect: Duration,
    /// Zeitgrenze für den gesamten Request (Verbindung + Antwort + Body).
    pub total: Duration,
}

impl Default for HttpTimeouts {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(3),
            total: Duration::from_secs(15),
        }
    }
}

impl HttpTimeouts {
    /// Baut einen `reqwest`-Client mit diesen Grenzen.
    ///
    /// Schlägt nur fehl, wenn das TLS-Backend nicht initialisierbar ist —
    /// das ist ein Startfehler, kein Laufzeitfehler, und bricht den Start
    /// sauber ab statt (wie `reqwest::Client::new()`) zu paniken.
    pub fn client(self) -> Result<reqwest::Client, BridgeError> {
        reqwest::Client::builder()
            .connect_timeout(self.connect)
            .timeout(self.total)
            .build()
            .map_err(|e| BridgeError::Config(format!("HTTP-Client nicht baubar: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_finite_and_ordered() {
        let t = HttpTimeouts::default();
        assert!(t.connect > Duration::ZERO);
        assert!(t.total > t.connect, "Gesamt-Timeout umfasst den Connect");
    }

    #[test]
    fn builds_a_client() {
        assert!(HttpTimeouts::default().client().is_ok());
    }
}
