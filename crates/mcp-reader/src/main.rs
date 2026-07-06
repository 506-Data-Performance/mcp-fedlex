//! mcp-reader - zustandsloser MCP-Reader (Direct Fetch).
//!
//! Binary-Entrypoint. Liest die Betriebsparameter aus der Umgebung, baut den
//! Dienst aus den Bausteinen der Bibliothek und serviert die zusammengesetzte
//! App ([`mcp_reader::app`]) hinter einem echten Socket. Die Routing- und
//! Kompositionslogik ist in der Lib per `cargo test` bewiesen. Diese Datei ist
//! nur die dünne Verkabelung aus Umgebung zu Netzwerk.
//!
//! Die Registry trägt die produktiven Navigations-Tools über der
//! fedlex-bridge (Direct Fetch gegen den Fedlex-SPARQL-Endpunkt und den
//! AKN-Filestore des Bundes). Credentials kommen wahlweise von einem IdP
//! (JWT, HS256/RS256 mit statischem Schlüsselmaterial) oder im Dev-Betrieb
//! aus MCP_DEV_TOKEN. Ohne beides bleibt der Server fail-closed.

use std::net::SocketAddr;
use std::sync::Arc;

use fedlex_bridge::{AknFetcher, HttpSparqlClient, HttpTimeouts, HttpXmlSource};
use mcp_reader::app::{RequestLimits, app, serve};
use mcp_reader::auth::{AuthResolver, JwksAuthResolver, JwtAuthResolver, StaticAuthResolver};
use mcp_reader::discovery::register_discovery_tools;
use mcp_reader::health::HealthState;
use mcp_reader::metadata::register_metadata_tools;

use mcp_reader::circuit_breaker::BreakerConfig;
use mcp_reader::probes::{QuotaBackendProbe, SparqlProbe};
use mcp_reader::quota::{QuotaPolicy, RateLimiter, RedisQuotaBackend};
use mcp_reader::registry::Registry;
use mcp_reader::resilience::{BreakeredSparql, BreakeredXml};
use mcp_reader::temporal::TemporalResolver;
use mcp_reader::tools::register_navigation_tools;

use mcp_reader::transport::McpService;
use tokio::net::TcpListener;

/// Byte-Budget des Manifestations-Caches (Summe der XML-Größen pro Pod).
/// Gewichtsbasiert statt zählbasiert (67 §H-5); via
/// MCP_FETCHER_CACHE_MAX_BYTES übersteuerbar.
const FETCHER_CACHE_MAX_BYTES: u64 = 256 * 1024 * 1024;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let addr: SocketAddr = bind_addr.parse()?;

    // Quota-Redis-Verbindung. Liegt Zertifikatsmaterial vor (cert-manager-
    // Volume, ADR-005), spricht der Reader gegenseitig authentifiziert (mTLS)
    // über `rediss://`. Ohne Material bleibt es bei der Klartext-Verbindung,
    // abgesichert durch die Default-Deny-NetworkPolicy.
    let backend = build_quota_backend(&redis_url)?;
    let limiter = RateLimiter::with_policy(backend.clone(), QuotaPolicy::default());

    // Zeitgrenzen für alle Upstream-Aufrufe (Fedlex-SPARQL, Filestore, JWKS).
    // Ohne sie bindet ein langsamer Upstream Tasks unbegrenzt (67 §H-1).
    let timeouts = upstream_timeouts_from_env()?;

    // Credential-Herkunft zur Laufzeit. JWT-Konfiguration gewinnt vor dem
    // Dev-Token. Ohne beides bleibt der Resolver leer (fail-closed, kein
    // einziges Credential gültig).
    let auth: Box<dyn AuthResolver + Send + Sync> = build_auth_resolver(timeouts)?;

    // Ein SPARQL-Client für alle Live-Pfade dieses Pods (reqwest teilt den
    // Connection-Pool über Clones), mit Zeitgrenzen aus der Umgebung — und
    // hinter einem gemeinsamen Circuit Breaker (67 §H-3): Fetcher, Discovery
    // und Metadaten teilen die Fehlerzähler; der Filestore trägt einen
    // eigenen Breaker. Die Readiness-Probe bleibt am rohen Client (sie soll
    // den echten Endpoint messen, nicht den Breaker-Zustand).
    let sparql_raw = HttpSparqlClient::fedlex_with(timeouts)?;
    let sparql = BreakeredSparql::new(sparql_raw.clone(), BreakerConfig::default());

    // Direct Fetch. Ein Fetcher (und damit ein Manifestations-Cache) für alle
    // Navigations-Tools dieses Pods.
    let cache_max_bytes = match std::env::var("MCP_FETCHER_CACHE_MAX_BYTES") {
        Ok(raw) => raw.parse().map_err(|_| {
            format!("MCP_FETCHER_CACHE_MAX_BYTES muss eine Byte-Zahl sein, war {raw:?}")
        })?,
        Err(_) => FETCHER_CACHE_MAX_BYTES,
    };
    let xml_max_bytes = match std::env::var("MCP_XML_MAX_BYTES") {
        Ok(raw) => raw
            .parse()
            .map_err(|_| format!("MCP_XML_MAX_BYTES muss eine Byte-Zahl sein, war {raw:?}"))?,
        Err(_) => fedlex_bridge::xml_source::DEFAULT_MAX_XML_BYTES,
    };
    let fetcher = Arc::new(AknFetcher::new(
        sparql.clone(),
        BreakeredXml::new(
            HttpXmlSource::with_timeouts(timeouts)?.with_max_bytes(xml_max_bytes),
            BreakerConfig::default(),
        ),
        cache_max_bytes,
    ));
    let mut registry = Registry::new();
    register_navigation_tools(&mut registry, fetcher);

    // Discovery-Tools (ADR-006). Live-Auflösung gegen Fedlex (Suche/SR-
    // Auflösung/Themen). Sie liefern Kandidaten-ELIs mit Hinweis-Provenance
    // und sind nur Navigator/Validator sichtbar.
    register_discovery_tools(&mut registry, Arc::new(sparql.clone()));

    // JOLux-Metadaten-Tools (ADR-007, Tranche A: Temporal). Sie belegen
    // Eigenschaften eines bekannten Erlasses (Norm-Provenance) und sind, wie
    // Discovery, nur Navigator/Validator sichtbar und im Quota gleich gewichtet.
    register_metadata_tools(&mut registry, Arc::new(sparql.clone()));

    // Default-Stichtag ist der Schweizer Kalendertag ZUR ANFRAGEZEIT (68 §F-6):
    // ein beim Start eingefrorenes «heute» veraltet mit jedem Lauftag des Pods
    // und stempelte real den Vortag.
    let temporal = TemporalResolver::swiss_today();
    let service = Arc::new(McpService::new(registry, auth, limiter, temporal));

    // Redis ist ready-kritisch (Quota); Fedlex nur informativ (67 §H-8):
    // Fällt der externe Upstream aus, bedienen Cache und lokale Navigation
    // weiter — alle Pods gleichzeitig unready zu nehmen, wäre eine
    // selbstgemachte Kaskade. Der Ausfall bleibt im readyz-Body sichtbar.
    let health = Arc::new(
        HealthState::new()
            .with_probe(Arc::new(QuotaBackendProbe::new(backend)))
            .with_informational_probe(Arc::new(SparqlProbe::new(sparql_raw))),
    );

    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "mcp-reader lauscht");

    // Kein Aufwärmlauf: Der Manifestations-Cache füllt sich lazy per
    // Single-Flight (67 §H-5). Ein Vorwärmen häufiger Erlasse (BV/OR/ZGB)
    // wäre ein bewusstes neues Feature, kein Restposten (67 §W-1).
    health.mark_started();

    let limits = request_limits_from_env()?;
    // Prometheus-Recorder (67 §O-2). /metrics liegt neben den Health-Routen
    // ausserhalb des Lastschutzes und wird am Ingress nicht öffentlich geroutet.
    let metrics_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .map_err(|e| format!("Prometheus-Recorder nicht installierbar: {e}"))?;
    let app = mcp_reader::app::with_metrics_route(
        app(service, Arc::clone(&health), limits),
        metrics_handle,
    );
    serve(listener, app).await?;
    Ok(())
}

/// Initialisiert strukturiertes Logging (67 §O-1).
///
/// Level via RUST_LOG (Default `info`); Format via MCP_LOG_FORMAT:
/// `text` (Default, lesbar für den 2-Minuten-Einstieg) oder `json`
/// (Produktion/K8s, gesetzt im Deployment). Die Audit-Zeile läuft als
/// `target: "audit"` weiter durch den PII-Scrubber — am Inhalt ändert
/// sich nichts, nur der Transport (vorher: `println!` mit stdout-Lock
/// auf dem Request-Pfad).
fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let json = matches!(
        std::env::var("MCP_LOG_FORMAT").as_deref(),
        Ok("json") | Ok("JSON")
    );
    if json {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }
}

/// Liest den Lastschutz der MCP-Routen aus der Umgebung (67 §H-2).
///
/// MCP_REQUEST_TIMEOUT_MS (Default 30000) und MCP_MAX_CONCURRENT_REQUESTS
/// (Default 256). Unparsebare Werte brechen den Start hart ab.
fn request_limits_from_env() -> Result<RequestLimits, Box<dyn std::error::Error>> {
    let defaults = RequestLimits::default();
    let timeout = match std::env::var("MCP_REQUEST_TIMEOUT_MS") {
        Ok(raw) => std::time::Duration::from_millis(raw.parse().map_err(|_| {
            format!("MCP_REQUEST_TIMEOUT_MS muss eine Millisekunden-Zahl sein, war {raw:?}")
        })?),
        Err(_) => defaults.timeout,
    };
    let max_concurrent = match std::env::var("MCP_MAX_CONCURRENT_REQUESTS") {
        Ok(raw) => raw.parse().map_err(|_| {
            format!("MCP_MAX_CONCURRENT_REQUESTS muss eine positive Zahl sein, war {raw:?}")
        })?,
        Err(_) => defaults.max_concurrent,
    };
    Ok(RequestLimits {
        timeout,
        max_concurrent,
    })
}

/// Liest die Upstream-Zeitgrenzen aus der Umgebung.
///
/// MCP_UPSTREAM_CONNECT_TIMEOUT_MS (Default 3000) und
/// MCP_UPSTREAM_TIMEOUT_MS (Default 15000). Unparsebare Werte brechen den
/// Start hart ab — eine stillschweigend ignorierte Fehlkonfiguration wäre
/// hier gefährlicher als ein klarer Startfehler.
fn upstream_timeouts_from_env() -> Result<HttpTimeouts, Box<dyn std::error::Error>> {
    fn ms(var: &str, default: u64) -> Result<std::time::Duration, Box<dyn std::error::Error>> {
        match std::env::var(var) {
            Ok(raw) => {
                let n: u64 = raw
                    .parse()
                    .map_err(|_| format!("{var} muss eine Millisekunden-Zahl sein, war {raw:?}"))?;
                Ok(std::time::Duration::from_millis(n))
            }
            Err(_) => Ok(std::time::Duration::from_millis(default)),
        }
    }
    Ok(HttpTimeouts {
        connect: ms("MCP_UPSTREAM_CONNECT_TIMEOUT_MS", 3_000)?,
        total: ms("MCP_UPSTREAM_TIMEOUT_MS", 15_000)?,
    })
}

/// Baut das Quota-Backend anhand der Umgebung.
///
/// Sind MCP_REDIS_TLS_CA_FILE, MCP_REDIS_TLS_CERT_FILE und
/// MCP_REDIS_TLS_KEY_FILE gesetzt (cert-manager-Volume, ADR-005), verbindet
/// sich der Reader gegenseitig authentifiziert (mTLS) über `rediss://`. Die
/// URL muss dann `rediss://` sein, sonst bricht der Start ab — eine
/// fehlkonfigurierte Klartext-URL bei vorhandenem Zertifikatsmaterial darf
/// nicht stillschweigend unverschlüsselt verbinden.
///
/// Ohne Zertifikatsmaterial bleibt es bei der bisherigen Klartext-Verbindung,
/// abgesichert durch die Default-Deny-NetworkPolicy. Das hält Dev- und
/// Migrationsbetrieb funktionsfähig.
fn build_quota_backend(redis_url: &str) -> Result<RedisQuotaBackend, Box<dyn std::error::Error>> {
    let ca = std::env::var("MCP_REDIS_TLS_CA_FILE").ok();
    let cert = std::env::var("MCP_REDIS_TLS_CERT_FILE").ok();
    let key = std::env::var("MCP_REDIS_TLS_KEY_FILE").ok();

    // Zeitgrenze pro Redis-Operation (67 §H-4): hängendes Redis fällt
    // fail-closed in den Fallback-Bucket statt Requests zu blockieren.
    let op_timeout = match std::env::var("MCP_REDIS_OP_TIMEOUT_MS") {
        Ok(raw) => std::time::Duration::from_millis(raw.parse().map_err(|_| {
            format!("MCP_REDIS_OP_TIMEOUT_MS muss eine Millisekunden-Zahl sein, war {raw:?}")
        })?),
        Err(_) => fedlex_store::token_bucket::DEFAULT_OP_TIMEOUT,
    };

    match (ca, cert, key) {
        (Some(ca), Some(cert), Some(key)) => {
            let tls = fedlex_store::RedisTlsConfig::from_files(&ca, &cert, &key)?;
            let backend =
                RedisQuotaBackend::connect_with_tls(redis_url, &tls)?.with_op_timeout(op_timeout);
            // RF-5: URL nur redigiert loggen — REDIS_URL trägt in Prod das
            // Passwort aus dem SealedSecret.
            let redis_url = fedlex_store::RedactedRedisUrl::new(redis_url);
            tracing::info!(%redis_url, "Quota-Redis über mTLS verbunden (ADR-005)");
            Ok(backend)
        }
        (None, None, None) => {
            let backend = RedisQuotaBackend::connect(redis_url)?.with_op_timeout(op_timeout);
            let redis_url = fedlex_store::RedactedRedisUrl::new(redis_url);
            tracing::info!(
                %redis_url,
                "Quota-Redis im Klartext verbunden; mTLS deaktiviert (kein Zertifikatsmaterial)"
            );
            Ok(backend)
        }
        _ => Err(
            "MCP_REDIS_TLS_{CA,CERT,KEY}_FILE müssen gemeinsam gesetzt sein \
                  (mTLS braucht CA, Client-Zert und Schlüssel)"
                .into(),
        ),
    }
}

/// Wählt den Auth-Resolver anhand der Umgebung.
///
/// Reihenfolge. Erst MCP_JWT_JWKS_URL (rotierende Schlüssel vom IdP), dann
/// MCP_JWT_HS256_SECRET, dann MCP_JWT_RS256_PUBKEY_FILE (PEM-Pfad), zuletzt
/// MCP_DEV_TOKEN. Im JWT-Modus ist MCP_JWT_ISSUER Pflicht und
/// MCP_JWT_AUDIENCE optional.
fn build_auth_resolver(
    timeouts: HttpTimeouts,
) -> Result<Box<dyn AuthResolver + Send + Sync>, Box<dyn std::error::Error>> {
    let issuer = std::env::var("MCP_JWT_ISSUER").ok();
    let audience = std::env::var("MCP_JWT_AUDIENCE").ok();

    if let Ok(url) = std::env::var("MCP_JWT_JWKS_URL") {
        let issuer = issuer.ok_or("MCP_JWT_ISSUER ist im JWT-Modus Pflicht")?;
        let refresh_secs: u64 = std::env::var("MCP_JWT_JWKS_REFRESH_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300);
        let resolver = Arc::new(JwksAuthResolver::new(issuer.clone(), audience));
        // Auch der JWKS-Abruf trägt die Upstream-Zeitgrenzen (H-1) — ein
        // hängender IdP darf den Refresh-Task nicht dauerhaft blockieren.
        let http = timeouts.client()?;
        spawn_jwks_refresher(Arc::clone(&resolver), http, url.clone(), refresh_secs);
        tracing::info!(
            jwks_url = url,
            issuer,
            refresh_secs,
            "JWT-Auth aktiv (JWKS)"
        );
        return Ok(Box::new(resolver));
    }

    if let Ok(secret) = std::env::var("MCP_JWT_HS256_SECRET") {
        let issuer = issuer.ok_or("MCP_JWT_ISSUER ist im JWT-Modus Pflicht")?;
        tracing::info!(issuer, "JWT-Auth aktiv (HS256)");
        return Ok(Box::new(JwtAuthResolver::hs256(
            secret.as_bytes(),
            &issuer,
            audience.as_deref(),
        )));
    }

    if let Ok(path) = std::env::var("MCP_JWT_RS256_PUBKEY_FILE") {
        let issuer = issuer.ok_or("MCP_JWT_ISSUER ist im JWT-Modus Pflicht")?;
        let pem = std::fs::read(&path)?;
        tracing::info!(issuer, key_file = path, "JWT-Auth aktiv (RS256)");
        return Ok(Box::new(
            JwtAuthResolver::rs256_pem(&pem, &issuer, audience.as_deref())
                .map_err(|_| format!("ungueltiger RSA-Public-Key in {path}"))?,
        ));
    }

    let mut auth = StaticAuthResolver::new();
    if let Ok(token) = std::env::var("MCP_DEV_TOKEN") {
        auth = auth.with_credential(
            token,
            mcp_reader::auth::ClaimRecord {
                tenant: "dev".into(),
                session: "dev".into(),
                role: mcp_reader::auth::Role::Validator,
            },
        );
        tracing::warn!("MCP_DEV_TOKEN aktiv (Rolle Validator, Mandant dev) — nicht für Produktion");
    } else {
        tracing::warn!(
            "Keine Auth-Konfiguration — Server bleibt fail-closed (kein Credential gueltig)"
        );
    }
    Ok(Box::new(auth))
}

/// Periodischer JWKS-Abruf. Erster Lauf sofort, danach im Intervall.
///
/// Fehler beim Abruf lassen den bisherigen Schlüsselsatz unangetastet
/// (Verfügbarkeit vor Frische). Bis zum ersten Erfolg ist der Satz leer
/// und der Resolver fail-closed.
fn spawn_jwks_refresher(
    resolver: Arc<JwksAuthResolver>,
    http: reqwest::Client,
    url: String,
    refresh_secs: u64,
) {
    tokio::spawn(async move {
        loop {
            match http.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => match resp.text().await {
                    Ok(body) => match resolver.install_jwks(&body) {
                        Ok(n) => tracing::info!(keys = n, "JWKS aktualisiert"),
                        Err(_) => tracing::warn!("JWKS nicht parsebar, alter Satz bleibt aktiv"),
                    },
                    Err(e) => tracing::warn!(error = %e, "JWKS-Body nicht lesbar"),
                },
                Ok(resp) => {
                    tracing::warn!(status = %resp.status(), "JWKS-Endpunkt antwortete nicht-2xx")
                }
                Err(e) => tracing::warn!(error = %e, "JWKS-Abruf fehlgeschlagen"),
            }
            tokio::time::sleep(std::time::Duration::from_secs(refresh_secs)).await;
        }
    });
}
