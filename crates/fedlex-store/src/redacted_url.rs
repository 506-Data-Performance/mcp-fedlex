//! Redis-URL mit redigierter Userinfo für Logzeilen (RF-5).
//!
//! `REDIS_URL` kommt in Prod aus einem SealedSecret in der Form
//! `rediss://default:<passwort>@host:6379` — eine rohe Ausgabe in einer
//! Logzeile trüge das Passwort in jeden Log-Store (Promtail → Loki) und in
//! Backups. Der allowlist-basierte Span-Scrubber (fedlex-telemetry) greift
//! für Span-Attribute, **nicht** für direkte `tracing::info!`-Felder — darum
//! wird hier am Entstehungspunkt redigiert: Typsystem statt Log-Disziplin,
//! analog `Sensitive<T>`.

use std::fmt;

/// Redis-URL, deren Userinfo (`user:passwort@`) bereits beim Konstruieren
/// durch `***` ersetzt ist. `Display` und `Debug` können das Passwort
/// strukturell nicht mehr zeigen, weil der Rohwert nie gespeichert wird.
pub struct RedactedRedisUrl(String);

impl RedactedRedisUrl {
    /// Redigiert die Userinfo der URL. URLs ohne Userinfo bleiben unverändert.
    pub fn new(url: &str) -> Self {
        let redacted = match url.split_once("://") {
            Some((scheme, rest)) => match rest.rsplit_once('@') {
                Some((_userinfo, host)) => format!("{scheme}://***@{host}"),
                None => url.to_owned(),
            },
            // Kein Schema (z.B. `host:port`): defensiv trotzdem redigieren.
            None => match url.rsplit_once('@') {
                Some((_userinfo, host)) => format!("***@{host}"),
                None => url.to_owned(),
            },
        };
        Self(redacted)
    }
}

impl fmt::Display for RedactedRedisUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for RedactedRedisUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RF-5-Regressionstest: das `:<pw>@`-Muster darf die Redaction nie
    /// überleben — weder über `Display` noch über `Debug`.
    #[test]
    fn password_never_survives_redaction() {
        let url = "rediss://default:super-geheim@mcp-reader-redis:6379";
        let redacted = RedactedRedisUrl::new(url);
        let shown = format!("{redacted} {redacted:?}");
        assert!(!shown.contains("super-geheim"), "war: {shown}");
        assert!(!shown.contains(":super-geheim@"), "war: {shown}");
        assert_eq!(
            redacted.to_string(),
            "rediss://***@mcp-reader-redis:6379",
            "Schema und Host müssen lesbar bleiben"
        );
    }

    #[test]
    fn url_without_userinfo_stays_unchanged() {
        let redacted = RedactedRedisUrl::new("redis://127.0.0.1:6379");
        assert_eq!(redacted.to_string(), "redis://127.0.0.1:6379");
    }

    #[test]
    fn url_without_scheme_is_still_redacted() {
        let redacted = RedactedRedisUrl::new("default:pw@host:6379");
        assert_eq!(redacted.to_string(), "***@host:6379");
    }
}
