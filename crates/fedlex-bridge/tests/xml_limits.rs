//! Abnahme 67 §H-6: Der XML-Download bricht über der Größen-Obergrenze ab —
//! sowohl beim angekündigten `Content-Length` (ohne den Body zu laden) als
//! auch bei einem Stream ohne Längenangabe (Kappung beim Empfang).

use fedlex_bridge::{BridgeError, HttpTimeouts, HttpXmlSource, XmlSource};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

/// Mini-HTTP-Server, der genau eine vorbereitete Rohantwort ausliefert.
async fn one_shot_server(raw_response: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            // Request-Header ignorieren, Antwort schreiben, Verbindung schliessen.
            let _ = socket.write_all(&raw_response).await;
            let _ = socket.shutdown().await;
        }
    });
    format!("http://{addr}")
}

fn source_with_limit(max_bytes: u64) -> HttpXmlSource {
    HttpXmlSource::with_timeouts(HttpTimeouts::default())
        .expect("source")
        .with_max_bytes(max_bytes)
}

/// Angekündigte Übergröße: Der Content-Length-Header reicht für den Abbruch,
/// der Body wird gar nicht erst gelesen.
#[tokio::test]
async fn oversized_content_length_is_rejected_before_download() {
    let response =
        b"HTTP/1.1 200 OK\r\nContent-Length: 1048576\r\nConnection: close\r\n\r\n".to_vec();
    let base = one_shot_server(response).await;

    let err = source_with_limit(1024)
        .fetch(&format!("{base}/x.xml"))
        .await
        .expect_err("angekuendigte Uebergroesse muss abgelehnt werden");
    assert!(
        matches!(err, BridgeError::Download(ref m) if m.contains("zu gross")),
        "erwartet Groessen-Fehler, war: {err:?}"
    );
}

/// Stream ohne Content-Length: Die Kappung greift beim Empfang.
#[tokio::test]
async fn oversized_stream_without_length_is_cut_off() {
    // HTTP/1.0-Stil: keine Content-Length, Body bis Verbindungsende.
    let mut response = b"HTTP/1.0 200 OK\r\nConnection: close\r\n\r\n".to_vec();
    response.extend(std::iter::repeat_n(b'x', 64 * 1024));
    let base = one_shot_server(response).await;

    let err = source_with_limit(4 * 1024)
        .fetch(&format!("{base}/x.xml"))
        .await
        .expect_err("Uebergroesse im Stream muss gekappt werden");
    assert!(
        matches!(err, BridgeError::Download(ref m) if m.contains("zu gross")),
        "erwartet Groessen-Fehler, war: {err:?}"
    );
}

/// Unter dem Limit fliesst der Body normal durch.
#[tokio::test]
async fn small_body_passes() {
    let body = "<akomaNtoso/>";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes();
    let base = one_shot_server(response).await;

    let xml = source_with_limit(1024)
        .fetch(&format!("{base}/x.xml"))
        .await
        .expect("kleiner Body muss durchgehen");
    assert_eq!(xml, body);
}
