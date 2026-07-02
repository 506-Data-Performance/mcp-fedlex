//! Breaker-Decorators für die Live-Pfade (67 §H-3).
//!
//! Legt den [`CircuitBreaker`] als Hülle um die produktiven Transport-Traits:
//! [`SparqlClient`] (Fedlex-SPARQL) und [`XmlSource`] (AKN-Filestore). Nach
//! `failure_threshold` Fehlern in Folge scheitern weitere Aufrufe **sofort**
//! (Fail-Fast) mit einer lenkenden Fehlermeldung, statt Tasks gegen einen
//! toten Endpunkt aufzustauen; nach dem Cooldown prüft ein einzelner
//! Probe-Aufruf die Genesung.
//!
//! SPARQL und Filestore tragen **getrennte** Breaker — der Filestore kann
//! gesund sein, während der SPARQL-Endpunkt klemmt (und umgekehrt).

use crate::circuit_breaker::{BreakerConfig, BreakerError, BreakerState, CircuitBreaker};
use async_trait::async_trait;
use fedlex_bridge::{BridgeError, XmlSource};
use fedlex_jolux::{JoluxError, SparqlClient, SparqlResults};
use std::sync::Arc;

/// Meldung, die Agenten über den Dispatch als `{error, hint}` erreicht.
const OPEN_MSG: &str = "Upstream voruebergehend gesperrt (zu viele Fehler in Folge, Circuit offen) — \
     der naechste Probe-Aufruf folgt automatisch nach dem Cooldown";

/// [`SparqlClient`] hinter einem Circuit Breaker.
///
/// `Clone` teilt den Breaker (`Arc`): Alle Klone — Fetcher, Discovery,
/// Metadaten — sehen dieselben Fehlerzähler und denselben Zustand. Genau so
/// bündelt sich die Fehlererkennung über alle Live-SPARQL-Pfade des Pods.
#[derive(Debug, Clone)]
pub struct BreakeredSparql<C> {
    inner: C,
    breaker: Arc<CircuitBreaker>,
}

impl<C> BreakeredSparql<C> {
    /// Umhüllt `inner` mit einem eigenen Breaker.
    pub fn new(inner: C, config: BreakerConfig) -> Self {
        Self {
            inner,
            breaker: Arc::new(CircuitBreaker::new(config)),
        }
    }

    /// Aktueller Breaker-Zustand (für Metriken/Diagnose).
    pub fn breaker_state(&self) -> BreakerState {
        self.breaker.state()
    }
}

#[async_trait]
impl<C: SparqlClient + Send + Sync> SparqlClient for BreakeredSparql<C> {
    async fn query(&self, sparql: &str) -> Result<SparqlResults, JoluxError> {
        self.breaker
            .call(|| self.inner.query(sparql))
            .await
            .map_err(|e| match e {
                BreakerError::Open => JoluxError::Transport(OPEN_MSG.into()),
                BreakerError::Upstream(inner) => inner,
            })
    }
}

/// [`XmlSource`] hinter einem Circuit Breaker.
#[derive(Debug)]
pub struct BreakeredXml<S> {
    inner: S,
    breaker: Arc<CircuitBreaker>,
}

impl<S> BreakeredXml<S> {
    /// Umhüllt `inner` mit einem eigenen Breaker.
    pub fn new(inner: S, config: BreakerConfig) -> Self {
        Self {
            inner,
            breaker: Arc::new(CircuitBreaker::new(config)),
        }
    }

    /// Aktueller Breaker-Zustand (für Metriken/Diagnose).
    pub fn breaker_state(&self) -> BreakerState {
        self.breaker.state()
    }
}

#[async_trait]
impl<S: XmlSource + Send + Sync> XmlSource for BreakeredXml<S> {
    async fn fetch(&self, url: &str) -> Result<String, BridgeError> {
        self.breaker
            .call(|| self.inner.fetch(url))
            .await
            .map_err(|e| match e {
                BreakerError::Open => BridgeError::Download(OPEN_MSG.into()),
                BreakerError::Upstream(inner) => inner,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    /// SPARQL-Client, der immer scheitert und die Aufrufe zählt.
    struct FailingSparql {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl SparqlClient for FailingSparql {
        async fn query(&self, _sparql: &str) -> Result<SparqlResults, JoluxError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(JoluxError::Transport("kaputt".into()))
        }
    }

    /// XML-Quelle, die immer scheitert und die Aufrufe zählt.
    struct FailingXml {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl XmlSource for FailingXml {
        async fn fetch(&self, _url: &str) -> Result<String, BridgeError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(BridgeError::Download("kaputt".into()))
        }
    }

    fn cfg(threshold: u32) -> BreakerConfig {
        BreakerConfig {
            failure_threshold: threshold,
            open_cooldown: Duration::from_secs(60),
        }
    }

    /// Abnahme 67 §H-3 (SPARQL): Nach dem Schwellwert schaltet der Breaker
    /// auf — weitere Aufrufe scheitern sofort mit lenkender Meldung, ohne den
    /// toten Endpunkt zu berühren.
    #[tokio::test]
    async fn sparql_short_circuits_after_threshold() {
        let client = BreakeredSparql::new(
            FailingSparql {
                calls: AtomicUsize::new(0),
            },
            cfg(3),
        );

        for _ in 0..3 {
            let err = client.query("SELECT 1").await.unwrap_err();
            assert!(matches!(err, JoluxError::Transport(ref m) if m == "kaputt"));
        }
        assert_eq!(client.breaker_state(), BreakerState::Open);

        for _ in 0..5 {
            let err = client.query("SELECT 1").await.unwrap_err();
            assert!(
                matches!(err, JoluxError::Transport(ref m) if m.contains("Circuit offen")),
                "erwartet Open-Meldung, war: {err:?}"
            );
        }
        assert_eq!(
            client.inner.calls.load(Ordering::SeqCst),
            3,
            "kein Aufruf darf den toten Endpunkt nach dem Öffnen erreichen"
        );
    }

    /// Abnahme 67 §H-3 (Filestore): identisches Verhalten für den XML-Pfad.
    #[tokio::test]
    async fn xml_short_circuits_after_threshold() {
        let source = BreakeredXml::new(
            FailingXml {
                calls: AtomicUsize::new(0),
            },
            cfg(2),
        );

        for _ in 0..2 {
            source.fetch("https://x/de/xml").await.unwrap_err();
        }
        assert_eq!(source.breaker_state(), BreakerState::Open);

        let err = source.fetch("https://x/de/xml").await.unwrap_err();
        assert!(
            matches!(err, BridgeError::Download(ref m) if m.contains("Circuit offen")),
            "erwartet Open-Meldung, war: {err:?}"
        );
        assert_eq!(source.inner.calls.load(Ordering::SeqCst), 2);
    }

    /// Erfolgreiche Aufrufe fliessen durch und halten den Breaker geschlossen.
    #[tokio::test]
    async fn success_passes_through() {
        struct OkSparql;
        #[async_trait]
        impl SparqlClient for OkSparql {
            async fn query(&self, _sparql: &str) -> Result<SparqlResults, JoluxError> {
                SparqlResults::from_json(r#"{"head":{"vars":[]},"results":{"bindings":[]}}"#)
            }
        }
        let client = BreakeredSparql::new(OkSparql, BreakerConfig::default());
        assert!(client.query("SELECT 1").await.is_ok());
        assert_eq!(client.breaker_state(), BreakerState::Closed);
    }
}
