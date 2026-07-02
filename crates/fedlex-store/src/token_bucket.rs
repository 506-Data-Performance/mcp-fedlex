//! Verteiltes, atomares Token-Bucket über Redis (ADR-002, Feature `redis-store`).
//!
//! Das Quota darf NICHT pod-lokal liegen. Sonst skaliert das Limit mit der
//! Pod-Zahl hoch und ein Agent umgeht es per Load-Balancing über mehrere Pods.
//! Deshalb lebt der gesamte Bucket-State in Redis und wird in EINEM Lua-Script
//! atomar gelesen, nachgefüllt, geprüft und zurückgeschrieben. Kein
//! Read-Modify-Write-Race zwischen Pods.

use redis::Script;
use redis::aio::ConnectionManager;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::OnceCell;

/// Default-Zeitgrenze pro Redis-Operation (67 §H-4).
///
/// Das Lua-Script braucht unter einer Millisekunde — 2 s sind großzügig.
/// Ohne diese Grenze schützt fail-closed nur gegen ein *totes* Redis
/// (schneller Fehler), nicht gegen ein *hängendes* (die Quota-Prüfung
/// blockiert dann den Request-Pfad, statt in den Fallback zu fallen).
pub const DEFAULT_OP_TIMEOUT: Duration = Duration::from_secs(2);

/// Parameter eines Token-Buckets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BucketParams {
    /// Maximale Tokenzahl (Burst-Grenze).
    pub capacity: u32,
    /// Nachfüllrate in Tokens pro Sekunde (darf gebrochen sein).
    pub refill_per_sec: f64,
    /// Lebensdauer des Bucket-Keys in Millisekunden (Aufräumen inaktiver Buckets).
    pub ttl_ms: u64,
}

/// Ergebnis eines Acquire-Versuchs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Acquisition {
    /// Ob die angeforderten Tokens gewährt wurden.
    pub allowed: bool,
    /// Verbleibende Tokens nach dem Versuch (abgerundet).
    pub remaining: i64,
    /// Wartezeit in Millisekunden bis genug Tokens vorhanden wären
    /// (`-1`, falls ohne Nachfüllung nie erreichbar).
    pub retry_after_ms: i64,
}

/// Atomares Token-Bucket über Redis.
///
/// `Clone` teilt die gemultiplexte Verbindung (`Arc<OnceCell<…>>`): Alle
/// Klone eines Buckets nutzen **eine** TCP-/TLS-Verbindung statt pro Aufruf
/// neu zu verbinden (67 §H-4 — mit mTLS wäre das ein Handshake pro Request).
#[derive(Clone)]
pub struct RedisTokenBucket {
    client: redis::Client,
    script: Script,
    conn: Arc<OnceCell<ConnectionManager>>,
    op_timeout: Duration,
}

/// Atomare Token-Bucket-Logik. Refill nach verstrichener Zeit, dann Prüfung
/// und Abbuchung, alles serverseitig in Redis ohne Race zwischen Pods.
const TOKEN_BUCKET_LUA: &str = r#"
local capacity = tonumber(ARGV[1])
local refill   = tonumber(ARGV[2])
local now      = tonumber(ARGV[3])
local cost     = tonumber(ARGV[4])
local ttl      = tonumber(ARGV[5])

local data   = redis.call('HMGET', KEYS[1], 'tokens', 'ts')
local tokens = tonumber(data[1])
local ts     = tonumber(data[2])
if tokens == nil then
  tokens = capacity
  ts = now
end

local elapsed = now - ts
if elapsed < 0 then elapsed = 0 end
tokens = math.min(capacity, tokens + (elapsed / 1000.0) * refill)

local allowed = 0
if tokens >= cost then
  tokens = tokens - cost
  allowed = 1
end

redis.call('HMSET', KEYS[1], 'tokens', tokens, 'ts', now)
redis.call('PEXPIRE', KEYS[1], ttl)

local retry = 0
if allowed == 0 then
  if refill > 0 then
    retry = math.ceil(((cost - tokens) / refill) * 1000.0)
  else
    retry = -1
  end
end

return {allowed, math.floor(tokens), retry}
"#;

impl RedisTokenBucket {
    /// Verbindet sich mit einer Redis-URL und lädt das Lua-Script.
    pub fn connect(url: &str) -> Result<Self, super::RedisError> {
        Ok(Self {
            client: redis::Client::open(url)?,
            script: Script::new(TOKEN_BUCKET_LUA),
            conn: Arc::new(OnceCell::new()),
            op_timeout: DEFAULT_OP_TIMEOUT,
        })
    }

    /// Verbindet sich gegenseitig authentifiziert (mTLS, ADR-005, Feature
    /// `redis-tls`). Die URL muss `rediss://` sein; das Client-Zertifikat aus
    /// `tls` weist den Reader gegenüber Redis aus, die CA prüft die Gegenseite.
    #[cfg(feature = "redis-tls")]
    pub fn connect_with_tls(
        url: &str,
        tls: &crate::redis_tls::RedisTlsConfig,
    ) -> Result<Self, super::RedisError> {
        Ok(Self {
            client: crate::redis_tls::build_tls_client(url, tls)?,
            script: Script::new(TOKEN_BUCKET_LUA),
            conn: Arc::new(OnceCell::new()),
            op_timeout: DEFAULT_OP_TIMEOUT,
        })
    }

    /// Setzt die Zeitgrenze pro Redis-Operation (Default [`DEFAULT_OP_TIMEOUT`]).
    pub fn with_op_timeout(mut self, op_timeout: Duration) -> Self {
        self.op_timeout = op_timeout;
        self
    }

    /// Liefert die geteilte, selbstheilende Verbindung (Lazy-Init).
    ///
    /// Der [`ConnectionManager`] reconnectet nach Verbindungsabbrüchen
    /// selbsttätig; schlägt die **erste** Verbindung fehl, bleibt die Zelle
    /// leer und der nächste Aufruf versucht es erneut.
    async fn manager(&self) -> Result<ConnectionManager, super::RedisError> {
        let conn = self
            .conn
            .get_or_try_init(|| ConnectionManager::new(self.client.clone()))
            .await?;
        Ok(conn.clone())
    }

    /// Versucht, `cost` Tokens aus dem Bucket `key` abzubuchen.
    ///
    /// `now_ms` ist die aktuelle Wall-Clock in Millisekunden. Der Aufruf ist
    /// atomar. Bei `allowed == false` ist `retry_after_ms` die geschätzte
    /// Wartezeit. Die gesamte Operation (inkl. Verbindungsaufbau) steht unter
    /// [`Self::with_op_timeout`] — ein hängendes Redis liefert einen Fehler,
    /// den der Aufrufer fail-closed behandelt, statt den Request zu blockieren.
    pub async fn try_acquire(
        &self,
        key: &str,
        params: BucketParams,
        cost: u32,
        now_ms: u64,
    ) -> Result<Acquisition, super::RedisError> {
        let op = async {
            let mut conn = self.manager().await?;
            let triple: (i64, i64, i64) = self
                .script
                .key(key)
                .arg(params.capacity)
                .arg(params.refill_per_sec)
                .arg(now_ms)
                .arg(cost)
                .arg(params.ttl_ms)
                .invoke_async(&mut conn)
                .await?;
            Ok::<_, super::RedisError>(triple)
        };

        let (allowed, remaining, retry_after_ms) = tokio::time::timeout(self.op_timeout, op)
            .await
            .map_err(|_| {
                super::RedisError::from(redis::RedisError::from(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!(
                        "Redis-Operation ueberschritt {} ms (haengendes Redis?)",
                        self.op_timeout.as_millis()
                    ),
                )))
            })??;

        Ok(Acquisition {
            allowed: allowed == 1,
            remaining,
            retry_after_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// Abnahme 67 §H-4: Ein *hängendes* Redis (Verbindung wird angenommen,
    /// aber nie beantwortet) läuft in das Op-Timeout, statt die Quota-Prüfung
    /// — und damit den Request-Pfad — unbegrenzt zu blockieren. Der Fehler
    /// landet beim Aufrufer, der ihn fail-closed behandelt (Fallback-Bucket).
    #[tokio::test]
    async fn hanging_redis_hits_op_timeout_instead_of_blocking() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let mut open = Vec::new();
            loop {
                if let Ok((socket, _)) = listener.accept().await {
                    open.push(socket); // offen halten, nie antworten
                }
            }
        });

        let bucket = RedisTokenBucket::connect(&format!("redis://{addr}"))
            .unwrap()
            .with_op_timeout(Duration::from_millis(150));
        let params = BucketParams {
            capacity: 10,
            refill_per_sec: 1.0,
            ttl_ms: 60_000,
        };

        let start = Instant::now();
        let err = bucket
            .try_acquire("tenant:sess:role", params, 1, 0)
            .await
            .expect_err("haengendes Redis muss einen Fehler liefern");
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_secs(2),
            "Abbruch muss in Timeout-Naehe erfolgen, dauerte {elapsed:?}"
        );
        assert!(
            err.to_string().contains("ueberschritt"),
            "erwartet Timeout-Fehler, war: {err}"
        );
    }

    /// Klone teilen die Verbindungs-Zelle — Grundlage des Connection-Reuse.
    #[tokio::test]
    async fn clones_share_the_connection_cell() {
        let a = RedisTokenBucket::connect("redis://127.0.0.1:6379").unwrap();
        let b = a.clone();
        assert!(Arc::ptr_eq(&a.conn, &b.conn));
    }
}
