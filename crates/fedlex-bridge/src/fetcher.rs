//! Die eigentliche Brücke. Komponiert JLX-TMP-02 (Stichtags-Auflösung der
//! Konsolidierung inkl. XML-URL) mit AKN-DOC-01 (Parse) und cached geparste
//! Dokumente pro Manifestations-URL.
//!
//! Die URL ist der ideale Cache-Schlüssel, weil sie pro Konsolidierung,
//! Sprache und Format eindeutig und unveränderlich ist (Manifestationen im
//! Filestore sind immutable). Verschiedene Stichtage, die auf dieselbe
//! Fassung auflösen, treffen damit denselben Cache-Eintrag.

use crate::error::BridgeError;
use crate::xml_source::XmlSource;
use fedlex_akn::AknDocument;
use fedlex_core::{Eli, Response, ValidAsOf};
use fedlex_jolux::{Language, SparqlClient, resolve_consolidation_at};
use moka::future::Cache;
use std::sync::Arc;

/// Beschafft AKN-Dokumente. Generisch über SPARQL-Transport und XML-Quelle —
/// produktiv [`HttpSparqlClient`] + [`HttpXmlSource`], im Test Mocks.
///
/// [`HttpSparqlClient`]: crate::HttpSparqlClient
/// [`HttpXmlSource`]: crate::HttpXmlSource
pub struct AknFetcher<C, S> {
    sparql: C,
    source: Arc<S>,
    /// Cache pro Manifestations-URL. Der Wert trägt die XML-Größe als
    /// Gewicht — das Budget ist ein **Byte-Budget**, keine Eintragszahl
    /// (67 §H-5: 64 Einträge à 1–10 MB waren ein unbegrenztes Speicherrisiko).
    cache: Cache<String, (Arc<AknDocument>, u32)>,
}

impl<C: SparqlClient, S: XmlSource + 'static> AknFetcher<C, S> {
    /// Fetcher mit Cache-Budget von `max_bytes` (Summe der XML-Größen).
    ///
    /// Richtwert: ein konsolidierter Erlass ist als XML grob 1–10 MB;
    /// 256 MB decken eine typische Agenten-Session bequem ab. Eviction ist
    /// gewichtsbasiert (TinyLFU), nicht zählbasiert.
    pub fn new(sparql: C, source: S, max_bytes: u64) -> Self {
        Self {
            sparql,
            source: Arc::new(source),
            cache: Cache::builder()
                .max_capacity(max_bytes)
                .weigher(|_key, (_doc, xml_size): &(Arc<AknDocument>, u32)| *xml_size)
                .build(),
        }
    }

    /// AKN-DOC-01 (produktiv): liefert das geparste AKN-Dokument eines
    /// Erlasses zum Stichtag in der gewünschten Sprache.
    ///
    /// Ablauf: JLX-TMP-02 löst Konsolidierung + XML-URL auf, dann Download +
    /// Parse — **Single-Flight** pro URL (67 §H-5): Bei N parallelen Misses
    /// auf dieselbe Manifestation lädt und parst genau einer, die übrigen
    /// warten auf sein Ergebnis; Fehler werden nicht gecacht, der nächste
    /// Aufruf versucht es erneut. Liefert [`JoluxError::NotFound`], wenn zum
    /// Stichtag keine XML-Manifestation existiert — XML gibt es im Filestore
    /// erst ab ~2021 (Live-Befund 2026-06-10, ältere Fassungen nur doc/pdf).
    ///
    /// Die Provenance stammt aus der JOLux-Auflösung (Work-ELI + Stichtag +
    /// Systemzeit, ADR-004).
    ///
    /// [`JoluxError::NotFound`]: fedlex_jolux::JoluxError::NotFound
    pub async fn fetch_akn_document(
        &self,
        eli: &Eli,
        as_of: ValidAsOf,
        lang: Language,
    ) -> Result<Response<Arc<AknDocument>>, BridgeError> {
        let cons = resolve_consolidation_at(&self.sparql, eli, as_of, lang).await?;
        let prov = cons.provenance().clone();
        let url = cons.data().xml_url.clone();

        let source = Arc::clone(&self.source);
        let init_url = url.clone();
        let (doc, _size) = self
            .cache
            .try_get_with(url, async move {
                let xml = source.fetch(&init_url).await?;
                let size = u32::try_from(xml.len()).unwrap_or(u32::MAX);
                let doc = Arc::new(AknDocument::parse(&xml)?);
                Ok::<_, BridgeError>((doc, size))
            })
            .await
            // moka teilt den Fehler als Arc an alle Wartenden; BridgeError
            // ist Clone, also bekommt jeder den strukturerhaltenden Fehler.
            .map_err(|e: Arc<BridgeError>| (*e).clone())?;

        Ok(Response::new(doc, prov))
    }
}
