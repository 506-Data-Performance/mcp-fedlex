//! Abnahme H-1 (67_HARDENING_AND_SOTA_ROADMAP): Die HTTP-Clients der Bridge
//! brechen gegen einen hängenden Upstream in der konfigurierten Zeit ab,
//! statt Tasks unbegrenzt zu binden.
//!
//! Der "Upstream" ist ein lokaler TCP-Listener, der Verbindungen annimmt,
//! aber nie antwortet — exakt das "Fedlex ist langsam"-Szenario (kein
//! externes Netz, läuft in jeder `cargo test`-Runde).

use fedlex_bridge::{HttpSparqlClient, HttpTimeouts, HttpXmlSource, XmlSource};
use fedlex_jolux::SparqlClient;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;

/// Startet einen Listener, der Verbindungen annimmt und offen hält,
/// ohne je ein Byte zu antworten.
async fn hanging_upstream() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let mut sockets = Vec::new();
        loop {
            if let Ok((socket, _)) = listener.accept().await {
                // Socket offen halten: der Client wartet vergeblich auf die Antwort.
                sockets.push(socket);
            }
        }
    });
    format!("http://{addr}")
}

fn short_timeouts() -> HttpTimeouts {
    HttpTimeouts {
        connect: Duration::from_millis(500),
        total: Duration::from_millis(300),
    }
}

#[tokio::test]
async fn sparql_client_aborts_against_hanging_upstream() {
    let endpoint = hanging_upstream().await;
    let client = HttpSparqlClient::with_timeouts(&endpoint, short_timeouts()).expect("client");

    let start = Instant::now();
    let err = client
        .query("SELECT (1 AS ?ok) WHERE {}")
        .await
        .expect_err("hängender Upstream muss einen Transportfehler liefern");
    let elapsed = start.elapsed();

    assert!(
        matches!(err, fedlex_jolux::JoluxError::Transport(_)),
        "erwartet Transport-Fehler, war: {err:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "Abbruch muss in Timeout-Nähe erfolgen, dauerte {elapsed:?}"
    );
}

#[tokio::test]
async fn xml_source_aborts_against_hanging_upstream() {
    let base = hanging_upstream().await;
    let source = HttpXmlSource::with_timeouts(short_timeouts()).expect("source");

    let start = Instant::now();
    let err = source
        .fetch(&format!("{base}/filestore/x/de/xml"))
        .await
        .expect_err("hängender Upstream muss einen Download-Fehler liefern");
    let elapsed = start.elapsed();

    assert!(
        matches!(err, fedlex_bridge::BridgeError::Download(_)),
        "erwartet Download-Fehler, war: {err:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "Abbruch muss in Timeout-Nähe erfolgen, dauerte {elapsed:?}"
    );
}

#[tokio::test]
async fn default_constructors_carry_timeouts() {
    // Die Default-Konstruktoren dürfen nie wieder einen grenzenlosen Client
    // bauen — Konstruktion muss gelingen und Default-Grenzen tragen.
    HttpSparqlClient::fedlex().expect("fedlex-Client mit Default-Timeouts");
    HttpXmlSource::new().expect("XML-Quelle mit Default-Timeouts");
    let t = HttpTimeouts::default();
    assert!(t.connect > Duration::ZERO && t.total > Duration::ZERO);
}
