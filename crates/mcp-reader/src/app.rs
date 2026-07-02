//! Server-Komposition. Vereint den SSE/JSON-RPC-Transport und die
//! Betriebs-Health-Endpunkte zu einer einzigen HTTP-App und serviert sie an
//! einem echten TcpListener.
//!
//! Bis hierher waren Transport ([`crate::transport`]) und Health
//! ([`crate::health`]) zwei getrennte Router ohne laufendes Binär. Erst die
//! Komposition macht den Dienst deploybar, denn die K8s-Probes brauchen reale
//! Pfade hinter einem lauschenden Socket.
//!
//! Der reine Zusammenbau ([`app`]) ist ohne Netzwerk per oneshot testbar.
//! [`serve`] bindet die App an einen Listener, sodass `/livez`, `/readyz` und
//! `/startupz` tatsächlich über HTTP antworten.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::error_handling::HandleErrorLayer;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower::limit::GlobalConcurrencyLimitLayer;
use tower::load_shed::LoadShedLayer;
use tower::timeout::TimeoutLayer;

use crate::auth::AuthResolver;
use crate::health::{HealthState, health_router};
use crate::quota::QuotaBackend;
use crate::transport::{McpService, router};

/// Lastschutz der MCP-Routen (67 §H-2).
///
/// Ohne diese Grenzen stauen sich Requests an hängenden Upstreams, bis
/// Tasks/Sockets/Speicher erschöpft sind. Der Timeout ist der Backstop
/// **über** den Upstream-Timeouts (H-1) — er greift, wenn alles andere
/// versagt; das Concurrency-Limit wirft Überlast sofort ab (Fail-Fast),
/// statt eine unsichtbare Warteschlange aufzubauen.
#[derive(Debug, Clone, Copy)]
pub struct RequestLimits {
    /// Harte Zeitgrenze pro Request (inkl. aller Upstream-Aufrufe).
    pub timeout: Duration,
    /// Maximal gleichzeitig bearbeitete MCP-Requests pro Pod.
    pub max_concurrent: usize,
}

impl Default for RequestLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_concurrent: 256,
        }
    }
}

/// Übersetzt Layer-Fehler (Timeout, Überlast) in lenkende JSON-Antworten —
/// im Stil der Quota-Antworten, damit Agenten sinnvoll reagieren können.
async fn handle_limit_error(err: tower::BoxError) -> Response {
    if err.is::<tower::timeout::error::Elapsed>() {
        (
            StatusCode::GATEWAY_TIMEOUT,
            axum::Json(json!({
                "error": "request timed out",
                "hint": "Der Aufruf ueberschritt die Server-Zeitgrenze. Versuche es erneut; \
                         dauert es wiederholt zu lange, verkleinere die Anfrage (z.B. einzelner \
                         Artikel statt ganzer Erlass)."
            })),
        )
            .into_response()
    } else {
        // LoadShed: Überlast — sofort abweisen, Client soll kurz warten.
        (
            StatusCode::SERVICE_UNAVAILABLE,
            [("Retry-After", "1")],
            axum::Json(json!({
                "error": "server overloaded",
                "hint": "Zu viele gleichzeitige Anfragen. Warte kurz und versuche es erneut.",
                "retry_after_ms": 1000
            })),
        )
            .into_response()
    }
}

/// Baut die vollständige HTTP-App.
///
/// Der Transport-Router (`/mcp`, `/rpc`, `/sse`) läuft hinter dem Lastschutz
/// ([`RequestLimits`]); die Health-Endpunkte (`/livez`, `/readyz`, `/startupz`)
/// bleiben bewusst **ausserhalb** — Probes müssen gerade unter Last antworten,
/// sonst killt Kubernetes den Pod genau dann, wenn er kämpft.
pub fn app<A, B>(
    service: Arc<McpService<A, B>>,
    health: Arc<HealthState>,
    limits: RequestLimits,
) -> Router
where
    A: AuthResolver + Send + Sync + 'static,
    B: QuotaBackend + Send + Sync + 'static,
{
    let protected = router(service).layer(
        ServiceBuilder::new()
            .layer(HandleErrorLayer::new(handle_limit_error))
            .layer(LoadShedLayer::new())
            .layer(GlobalConcurrencyLimitLayer::new(limits.max_concurrent))
            .layer(TimeoutLayer::new(limits.timeout)),
    );
    protected.merge(health_router(health))
}

/// Serviert eine fertige App am Listener bis zum Prozessende.
///
/// Dünne Hülle um `axum::serve`. Der Listener ist bewusst von aussen
/// hereingereicht, damit Aufrufer (Tests wie `main`) die Adresse selbst wählen.
pub async fn serve(listener: TcpListener, app: Router) -> std::io::Result<()> {
    axum::serve(listener, app).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{ClaimRecord, Role, StaticAuthResolver};
    use crate::quota::{QuotaError, QuotaPolicy, RateLimiter};
    use crate::registry::Registry;
    use crate::temporal::TemporalResolver;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use fedlex_store::token_bucket::{Acquisition, BucketParams};
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tower::ServiceExt;

    /// Quota-Backend, das im Test stets erlaubt.
    struct AllowingBackend;

    impl QuotaBackend for AllowingBackend {
        async fn try_acquire(
            &self,
            _key: &str,
            _params: BucketParams,
            _cost: u32,
            _now_ms: u64,
        ) -> Result<Acquisition, QuotaError> {
            Ok(Acquisition {
                allowed: true,
                remaining: 99,
                retry_after_ms: 0,
            })
        }
    }

    fn service() -> Arc<McpService<StaticAuthResolver, AllowingBackend>> {
        let registry = Registry::new();
        let auth = StaticAuthResolver::new().with_credential(
            "token-a",
            ClaimRecord {
                tenant: "kanzlei-a".into(),
                session: "sess-1".into(),
                role: Role::Reader,
            },
        );
        let limiter = RateLimiter::with_policy(AllowingBackend, QuotaPolicy::default());
        let temporal = TemporalResolver::new(time::macros::date!(2024 - 01 - 01));
        Arc::new(McpService::new(registry, auth, limiter, temporal))
    }

    /// Test-Tool, das kontrolliert langsam antwortet (für Timeout/Overload).
    struct SlowTool {
        delay: Duration,
    }

    #[async_trait::async_trait]
    impl crate::tool::McpTool for SlowTool {
        fn name(&self) -> &str {
            "slow_probe"
        }
        fn pool(&self) -> crate::tool::ToolPool {
            crate::tool::ToolPool::LocalNavigation
        }
        fn schema(&self) -> serde_json::Value {
            json!({"type": "object", "properties": {}})
        }
        async fn execute(
            &self,
            _ctx: &crate::tool::ToolContext,
            _args: serde_json::Value,
        ) -> Result<fedlex_core::Response<serde_json::Value>, crate::tool::ToolError> {
            tokio::time::sleep(self.delay).await;
            let prov = fedlex_core::Provenance::new(
                fedlex_core::Eli::new("eli/cc/2017/762").unwrap(),
                fedlex_core::ValidAsOf::new(time::macros::date!(2024 - 01 - 01)),
                fedlex_core::TransactionTime::new(time::OffsetDateTime::now_utc()),
            );
            Ok(fedlex_core::Response::new(json!({"ok": true}), prov))
        }
    }

    /// App mit einem langsamen Tool und gegebenen Limits.
    fn slow_app(delay: Duration, limits: RequestLimits) -> Router {
        let mut registry = Registry::new();
        registry.register(Arc::new(SlowTool { delay }));
        let auth = StaticAuthResolver::new().with_credential(
            "token-a",
            ClaimRecord {
                tenant: "kanzlei-a".into(),
                session: "sess-1".into(),
                role: Role::Reader,
            },
        );
        let limiter = RateLimiter::with_policy(AllowingBackend, QuotaPolicy::default());
        let temporal = TemporalResolver::new(time::macros::date!(2024 - 01 - 01));
        let service = Arc::new(McpService::new(registry, auth, limiter, temporal));
        app(service, Arc::new(HealthState::new()), limits)
    }

    fn call_slow_probe() -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/rpc")
            .header("authorization", "Bearer token-a")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"slow_probe","arguments":{}}}"#,
            ))
            .unwrap()
    }

    /// Abnahme 67 §H-2: Ein Handler, der die Zeitgrenze reisst, wird hart
    /// beendet und liefert eine lenkende 504-Antwort statt endlos zu hängen.
    #[tokio::test]
    async fn request_timeout_cuts_slow_handler() {
        let app = slow_app(
            Duration::from_secs(30),
            RequestLimits {
                timeout: Duration::from_millis(100),
                max_concurrent: 8,
            },
        );
        let start = std::time::Instant::now();
        let resp = app.oneshot(call_slow_probe()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::GATEWAY_TIMEOUT);
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "Timeout muss den Request in Grenzennähe kappen"
        );
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("hint"), "lenkende Antwort erwartet: {text}");
    }

    /// Abnahme 67 §H-2: Überlast wird sofort abgeworfen (Fail-Fast, 503 +
    /// Retry-After) statt eine unsichtbare Warteschlange aufzubauen.
    #[tokio::test]
    async fn overload_is_shed_with_503() {
        let app = slow_app(
            Duration::from_millis(300),
            RequestLimits {
                timeout: Duration::from_secs(10),
                max_concurrent: 1,
            },
        );

        let first = app.clone().oneshot(call_slow_probe());
        let second = async {
            // Der zweite Request startet, während der erste den einzigen
            // Slot belegt.
            tokio::time::sleep(Duration::from_millis(50)).await;
            app.clone().oneshot(call_slow_probe()).await
        };
        let (first, second) = tokio::join!(first, second);

        let statuses = [first.unwrap().status(), second.unwrap().status()];
        assert!(
            statuses.contains(&StatusCode::OK),
            "einer muss durchkommen: {statuses:?}"
        );
        assert!(
            statuses.contains(&StatusCode::SERVICE_UNAVAILABLE),
            "einer muss geshedded werden: {statuses:?}"
        );
    }

    /// Health-Probes bleiben ausserhalb des Lastschutzes: Auch wenn alle
    /// MCP-Slots belegt sind, antwortet /livez sofort.
    #[tokio::test]
    async fn health_stays_reachable_under_full_load() {
        let app = slow_app(
            Duration::from_millis(500),
            RequestLimits {
                timeout: Duration::from_secs(10),
                max_concurrent: 1,
            },
        );

        let blocker = app.clone().oneshot(call_slow_probe());
        let livez = async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let start = std::time::Instant::now();
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/livez")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            (resp.status(), start.elapsed())
        };
        let (blocker, (livez_status, livez_elapsed)) = tokio::join!(blocker, livez);

        assert_eq!(blocker.unwrap().status(), StatusCode::OK);
        assert_eq!(livez_status, StatusCode::OK);
        assert!(
            livez_elapsed < Duration::from_millis(400),
            "livez darf nicht hinter dem MCP-Slot warten, brauchte {livez_elapsed:?}"
        );
    }

    #[tokio::test]
    async fn merged_app_serves_health_and_transport() {
        let health = Arc::new(HealthState::new());
        let app = app(service(), Arc::clone(&health), RequestLimits::default());

        let livez = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/livez")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(livez.status(), StatusCode::OK);

        let sse = app
            .oneshot(Request::builder().uri("/sse").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(sse.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn startup_flips_to_ready_only_after_mark_started() {
        let health = Arc::new(HealthState::new());
        let app = app(service(), Arc::clone(&health), RequestLimits::default());

        let before = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/startupz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(before.status(), StatusCode::SERVICE_UNAVAILABLE);

        health.mark_started();

        let after = app
            .oneshot(
                Request::builder()
                    .uri("/startupz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(after.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn served_over_a_real_socket_answers_livez() {
        let health = Arc::new(HealthState::new());
        let app = app(service(), Arc::clone(&health), RequestLimits::default());

        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            serve(listener, app).await.unwrap();
        });

        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(b"GET /livez HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();

        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        let text = String::from_utf8_lossy(&response);
        assert!(text.starts_with("HTTP/1.1 200 OK"), "Antwort war: {text}");
        assert!(text.contains("\"status\":\"alive\""), "Antwort war: {text}");
    }
}
