//! MCP-Registry. Tool-Pool-Verwaltung, RBAC-gefiltertes `tools/list`, Dispatch
//! und die globale Graceful-Failure-Middleware (ADR-004).
//!
//! Zwei Garantien dieser Schicht.
//! 1. `tools/list` liefert pro Rolle NUR den erlaubten Pool (Least-Privilege).
//! 2. `dispatch` gibt IMMER valides JSON zurück. Erfolg ist ein
//!    `{ data, provenance }` aus [`Response`] (Provenance strukturell erzwungen),
//!    jeder Fehler wird zu einem lenkenden `{ error, hint }`. Niemals ein
//!    roher Crash ans LLM.

use crate::auth::Role;
use crate::tool::{McpTool, ToolContext, ToolPool, role_allows};

use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Zentrale Registry der MCP-Tools.
#[derive(Default, Clone)]
pub struct Registry {
    tools: BTreeMap<String, Arc<dyn McpTool>>,
}

impl Registry {
    /// Erzeugt eine leere Registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registriert ein Tool. Ein gleichnamiges Tool wird überschrieben.
    pub fn register(&mut self, tool: Arc<dyn McpTool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Der RBAC-Pool eines registrierten Tools, falls bekannt. Wird vom
    /// Transport gebraucht, um das pool-abhängige Quota-Gewicht (ADR-006)
    /// VOR dem Dispatch zu bestimmen.
    pub fn pool_of(&self, name: &str) -> Option<ToolPool> {
        self.tools.get(name).map(|t| t.pool())
    }

    /// `tools/list` für eine Rolle. Liefert nur Schemas der erlaubten Pools.
    ///
    /// **Wire-Form (ADR-008 §A, additive Migration):** Jeder Eintrag trägt das
    /// Tool-Schema unter ZWEI Schlüsseln:
    /// - `inputSchema` — der MCP-Standardschlüssel der Ziel-Revision 2025-11-25.
    /// - `schema` — der historische Schlüssel dieses Readers (Alt-Client-Vertrag).
    ///
    /// Beide tragen denselben Wert. Das ist die Brücke der Migrations-Phase: neue
    /// (standardkonforme) Clients lesen `inputSchema`, der handshake-lose Alt-Client
    /// ansV liest weiterhin `schema` und bleibt unberührt. Der Legacy-Schlüssel
    /// wird erst entfernt, wenn alle Clients umgestellt sind (Runbook Phase 9);
    /// dieses Doppel-Emit ist bewusst temporär.
    pub fn list_tools(&self, role: Role) -> Vec<Value> {
        self.tools
            .values()
            .filter(|t| role_allows(role, t.pool()))
            .map(|t| {
                let mut schema = t.schema();
                // Stichtag zentral annoncieren (68 §A-2, ADR-011): JEDE Antwort
                // trägt ein `valid_as_of` in der Provenance — der Stichtag ist
                // damit eine Dimension jedes Aufrufs, nicht einzelner Tools.
                // Zentral injiziert statt 40-fach von Hand gepflegt (keine
                // neue Drift-Quelle); ein Tool-eigenes `as_of` würde gewinnen.
                if let Some(props) = schema
                    .get_mut("properties")
                    .and_then(Value::as_object_mut)
                {
                    props.entry("as_of").or_insert_with(|| {
                        json!({
                            "type": "string",
                            "format": "date",
                            "pattern": "^\\d{4}-\\d{2}-\\d{2}$",
                            "description": "Stichtag JJJJ-MM-TT (optional; Default: heute, Schweizer Zeit). Bestimmt die geltende Fassung; das effektiv verwendete Datum steht in provenance.valid_as_of.",
                        })
                    });
                }
                // Top-Level-`description` (68 §A-1): MCP-Hosts präsentieren dem
                // Modell DIESES Feld — nicht `inputSchema.description`. Ohne das
                // Doppel-Emit erschienen alle Tools in Standard-Hosts nackt.
                let description = schema.get("description").cloned().unwrap_or(Value::Null);
                json!({
                    "name": t.name(),
                    "description": description,
                    // Standardschlüssel (Ziel-Revision) und Legacy-Schlüssel
                    // tragen denselben Wert — siehe Doc-Kommentar oben.
                    "inputSchema": schema,
                    "schema": schema,
                    // Tool-Annotations (67 §P-2). Zentral gesetzt, weil sie
                    // eine Architektur-Invariante spiegeln: Der Reader ist die
                    // CQRS-Leseseite — JEDES Tool ist read-only und idempotent
                    // (gleiche Anfrage + Stichtag = gleiche Antwort), und der
                    // Fedlex-Korpus ist eine geschlossene Domäne (kein Open-
                    // World-Verhalten wie Websuche). Käme je ein schreibendes
                    // Tool, müsste das McpTool-Trait Annotations liefern.
                    "annotations": {
                        "readOnlyHint": true,
                        "idempotentHint": true,
                        "openWorldHint": false,
                    },
                    // outputSchema (ADR-009): zentral, weil strukturell
                    // erzwungen — jede Erfolgsantwort ist {data, provenance}
                    // mit exakter Provenance-Form (ADR-004/006). `data` bleibt
                    // bewusst tool-spezifisch untypisiert (kein handgepflegtes
                    // Schema je Tool als nächste Drift-Quelle).
                    "outputSchema": output_schema(),
                })
            })
            .collect()
    }

    /// `tools/call`. Führt ein Tool aus und liefert immer valides JSON.
    ///
    /// Reihenfolge der Prüfungen. Existenz, dann RBAC, dann Ausführung. Jeder
    /// Fehlerpfad endet in der Graceful-Failure-Hülle.
    pub async fn dispatch(&self, ctx: &ToolContext, name: &str, args: Value) -> Value {
        let role = ctx.claims.role();

        let Some(tool) = self.tools.get(name) else {
            return graceful(
                &format!("unknown tool `{name}`"),
                "Rufe zuerst tools/list auf und waehle einen verfuegbaren Tool-Namen.",
            );
        };

        // Least-Privilege. Eine Rolle darf fremde Pools nicht aufrufen, selbst
        // wenn sie den Namen errät.
        if !role_allows(role, tool.pool()) {
            return graceful(
                &format!("tool `{name}` not permitted for this role"),
                "Dieses Tool steht deiner Rolle nicht zur Verfuegung.",
            );
        }

        match tool.execute(ctx, args).await {
            // Erfolg. Die Serialisierung von Response enthält zwingend die
            // Provenance. Das ist der strukturelle ADR-004-Nachweis im Wire-Format.
            Ok(response) => match serde_json::to_value(&response) {
                Ok(value) => value,
                Err(e) => graceful(
                    &format!("serialization failed: {e}"),
                    "Interner Fehler beim Verpacken der Antwort.",
                ),
            },
            Err(err) => graceful(&err.to_string(), err.hint()),
        }
    }
}

/// Das gemeinsame `outputSchema` aller Tools (ADR-009): beschreibt den
/// Erfolgsfall (`isError: false`) — Nutzdaten plus strukturell garantierte
/// Provenance (ADR-004; `kind` unterscheidet Norm-Beleg und Discovery-Hinweis,
/// ADR-006).
fn output_schema() -> Value {
    json!({
        "type": "object",
        "required": ["data", "provenance"],
        "properties": {
            "data": {
                "description": "Tool-spezifische Nutzdaten (Form siehe Tool-Beschreibung)."
            },
            "provenance": {
                "type": "object",
                "required": ["kind", "eli", "valid_as_of", "transaction_time"],
                "properties": {
                    "kind": { "type": "string", "enum": ["norm", "hint"] },
                    "eli": { "type": "string" },
                    "valid_as_of": { "type": "string", "description": "Stichtag (JJJJ-MM-TT)" },
                    "transaction_time": { "type": "string", "description": "Abrufzeitpunkt" },
                    "date_applicability": { "type": "string", "description": "Stand-Datum der tatsaechlich aufgeloesten Fassung (JJJJ-MM-TT). Weicht valid_as_of davon ab (z.B. kuenftiger Stichtag), traegt die Aussage nur diese Fassung." }
                }
            }
        }
    })
}

/// Verpackt eine Nutzlast in die `CallToolResult`-Hülle der MCP-Spec (ADR-009).
///
/// `structuredContent` trägt das Domänen-Objekt unverändert (`{data, provenance}`
/// bzw. `{error, hint}` — der ADR-004-Vertrag bleibt byte-gleich, nur eine Ebene
/// tiefer); `content[0]` dieselbe Nutzlast als serialisierten JSON-Text für
/// Clients ohne `structuredContent`-Support. Die Hülle ist ein reines
/// Wire-Format-Detail: `dispatch` und die Tool-Tests arbeiten auf der rohen
/// Nutzlast, der Transport verpackt am Rand.
pub(crate) fn call_tool_result(payload: Value, is_error: bool) -> Value {
    let text = payload.to_string();
    json!({
        "content": [ { "type": "text", "text": text } ],
        "structuredContent": payload,
        "isError": is_error,
    })
}

/// Baut die lenkende Fehler-Hülle für das LLM.
fn graceful(error: &str, hint: &str) -> Value {
    json!({ "error": error, "hint": hint })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthResolver, ClaimRecord, StaticAuthResolver, VerifiedClaims};
    use crate::temporal::TemporalResolver;
    use crate::tool::{ToolError, ToolPool};
    use async_trait::async_trait;
    use fedlex_core::{Eli, Response, TransactionTime};
    use serde_json::json;
    use time::macros::{date, datetime};

    fn ctx(role: Role) -> ToolContext {
        let claims: VerifiedClaims = StaticAuthResolver::new()
            .with_credential(
                "c",
                ClaimRecord {
                    tenant: "kanzlei-a".into(),
                    session: "sess-1".into(),
                    role,
                },
            )
            .verify("c")
            .unwrap();
        let stamp = TemporalResolver::new(date!(2024 - 01 - 01)).stamp_at(
            Some(date!(2020 - 01 - 01)),
            TransactionTime::new(datetime!(2026-06-01 09:00 UTC)),
        );
        ToolContext { claims, stamp }
    }

    /// Ein wohlerzogenes Tool. Baut die Provenance aus dem Anfrage-Stempel.
    struct ReadArticle;

    #[async_trait]
    impl McpTool for ReadArticle {
        fn name(&self) -> &str {
            "read_article"
        }
        fn pool(&self) -> ToolPool {
            ToolPool::LocalNavigation
        }
        fn schema(&self) -> Value {
            json!({ "type": "object", "properties": { "eli": { "type": "string" } } })
        }
        async fn execute(
            &self,
            ctx: &ToolContext,
            _args: Value,
        ) -> Result<Response<Value>, ToolError> {
            let eli = Eli::new("eli/cc/1999/404")
                .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;
            let prov = ctx.stamp.into_provenance(eli);
            Ok(Response::new(json!({ "text": "Art. 1 BV" }), prov))
        }
    }

    /// Ein Validierungs-Tool. Nur für Validator-Rolle sichtbar.
    struct ValidateSchema;

    #[async_trait]
    impl McpTool for ValidateSchema {
        fn name(&self) -> &str {
            "validate_schema"
        }
        fn pool(&self) -> ToolPool {
            ToolPool::Validation
        }
        fn schema(&self) -> Value {
            json!({ "type": "object" })
        }
        async fn execute(
            &self,
            _ctx: &ToolContext,
            _args: Value,
        ) -> Result<Response<Value>, ToolError> {
            Err(ToolError::Upstream("validator offline".into()))
        }
    }

    fn registry() -> Registry {
        let mut r = Registry::new();
        r.register(Arc::new(ReadArticle));
        r.register(Arc::new(ValidateSchema));
        r
    }

    #[tokio::test]
    async fn list_tools_is_role_filtered_least_privilege() {
        let r = registry();

        let reader: Vec<_> = r
            .list_tools(Role::Reader)
            .into_iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(reader, vec!["read_article"]); // kein validate_schema

        let validator: Vec<_> = r
            .list_tools(Role::Validator)
            .into_iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        assert!(validator.contains(&"read_article".to_string()));
        assert!(validator.contains(&"validate_schema".to_string()));
    }

    #[tokio::test]
    async fn successful_dispatch_carries_provenance_in_wire_format() {
        let r = registry();
        let out = r
            .dispatch(&ctx(Role::Reader), "read_article", json!({}))
            .await;

        // ADR-004. Die Antwort trägt strukturell ihre Herkunft.
        assert_eq!(out["data"]["text"], "Art. 1 BV");
        assert_eq!(out["provenance"]["eli"], "eli/cc/1999/404");
        assert!(
            !out["provenance"]["valid_as_of"].is_null(),
            "valid_as_of muss in der Herkunft gesetzt sein"
        );
        assert!(
            !out["provenance"]["transaction_time"].is_null(),
            "transaction_time muss in der Herkunft gesetzt sein"
        );
        assert!(out.get("error").is_none());
    }

    #[tokio::test]
    async fn unknown_tool_yields_graceful_failure_not_panic() {
        let r = registry();
        let out = r
            .dispatch(&ctx(Role::Reader), "drop_database", json!({}))
            .await;
        assert!(out["error"].as_str().unwrap().contains("unknown tool"));
        assert!(out["hint"].is_string());
    }

    #[tokio::test]
    async fn forbidden_pool_is_denied_for_role() {
        let r = registry();
        // Reader kennt validate_schema nicht und darf es auch nicht aufrufen.
        let out = r
            .dispatch(&ctx(Role::Reader), "validate_schema", json!({}))
            .await;
        assert!(out["error"].as_str().unwrap().contains("not permitted"));
    }

    #[tokio::test]
    async fn tool_error_is_translated_to_hint() {
        let r = registry();
        // Validator darf validate_schema, das Tool meldet aber Upstream-Fehler.
        let out = r
            .dispatch(&ctx(Role::Validator), "validate_schema", json!({}))
            .await;
        assert!(out["error"].as_str().unwrap().contains("upstream failure"));
        assert!(out["hint"].as_str().unwrap().contains("nachgelagerter"));
    }
}
