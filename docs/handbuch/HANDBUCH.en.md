# mcp-fedlex — The Handbook

🇬🇧 English · [🇩🇪 Deutsch](./HANDBUCH.de.md)

**Explained clearly, from the ground up.**

> **What this document is.** The plain-language documentation of mcp-fedlex: it explains,
> without assuming prior knowledge, what the system is, how it works, why its answers
> can be trusted, and where its limits lie — including all 40 tools, a large
> question-and-answer section, and a glossary. It is readable as a stand-alone document
> and intended for PDF export (see [`docs/README.md`](../README.md)).
>
> **What it is not:** a developer reference. Those who implement, configure, or
> operate the system will find the authoritative technical documents in the
> [documentation index](../README.md).

As of: 2026-07-08 · Software version: v0.2.0 · License: Apache-2.0 © [mindful.bio](https://mindful.bio)

---

**Contents**

*Part I — Understanding*

- [1. What is this about?](#1-what-is-this-about)
- [2. The basic concepts, explained simply](#2-the-basic-concepts-explained-simply)
- [3. How a request is processed](#3-how-a-request-is-processed)
- [4. Norm citation or discovery hint — the centerpiece](#4-norm-citation-or-discovery-hint--the-centerpiece)
- [5. The point-in-time date — the built-in time machine](#5-the-point-in-time-date--the-built-in-time-machine)

*Part II — The Tools*

- [6. Four toolboxes, three roles](#6-four-toolboxes-three-roles)
- [7. All 40 tools in detail](#7-all-40-tools-in-detail)
- [8. Typical workflows](#8-typical-workflows)

*Part III — Trust, Security, Limits*

- [9. Why the answers can be trusted](#9-why-the-answers-can-be-trusted)
- [10. Who may do what — identity and roles](#10-who-may-do-what--identity-and-roles)
- [11. Rate limiting (quota)](#11-rate-limiting-quota)
- [12. Data protection and logging](#12-data-protection-and-logging)
- [13. The limits of the system — honestly stated](#13-the-limits-of-the-system--honestly-stated)

*Part IV — Try It Yourself*

- [14. Start locally in two minutes](#14-start-locally-in-two-minutes)
- [15. Click through in the browser (MCP Inspector)](#15-click-through-in-the-browser-mcp-inspector)
- [16. Connecting an AI client](#16-connecting-an-ai-client)
- [17. Running in production — the overview](#17-running-in-production--the-overview)

*Part V — Reference*

- [18. Questions and answers (FAQ)](#18-questions-and-answers-faq)
- [19. Glossary](#19-glossary)
- [20. Further documents](#20-further-documents)

---

# Part I — Understanding

## 1. What is this about?

**The problem.** Language models ("AI", e.g. behind chat assistants) formulate
answers from what they learned during training. For legal questions this is
dangerous: the model sounds convincing, but it can invent articles, reproduce
outdated versions, or mix up laws — and nobody can tell from the answer where it
came from. For legal work, an answer without a verifiable source is worthless.

**The solution.** mcp-fedlex is a server that puts **tools** into a language model's
hands so it can **look things up in Swiss federal law instead of inventing them**.
Think of it as a meticulously careful librarian:

- It fetches the **actual legal text** from Fedlex, the official publication platform
  of federal law — not from the model's memory.
- It delivers every text **as of a specific point-in-time date** ("What was in force
  on 1 January 2019?"), not just "some version or other".
- It attaches a **provenance record to every answer** (which act, which point-in-time
  date) that the model cannot forge.
- It distinguishes structurally between a **norm citation** (this is the text of the
  norm) and a **discovery hint** (this could be relevant — please verify).

**In one sentence:** mcp-fedlex makes Swiss federal law accessible to AI systems in
such a way that every statement is **citable, point-in-time accurate, and verifiable**.

**What it deliberately is not:** not a chatbot, not legal advice, not a legal
database of its own. It is the controlled bridge between an AI and the public,
official Fedlex data.

**Who is behind it?** mcp-fedlex is a product of the company
[mindful.bio](https://mindful.bio). It is the data layer of a small product family:
the application platform **[ansV](https://ansv.ch)** builds on it and produces
legal analyses with a traceable chain of citations; the five-language project
description is available at **[mcp-fedlex.ch](https://mcp-fedlex.ch)**. The source
code is open under the Apache-2.0 license.

## 2. The basic concepts, explained simply

These eight concepts are enough to understand everything that follows.

**Fedlex** — the official online publication platform of Swiss federal law,
operated by the federal government. It contains all federal acts, ordinances, and
international treaties — publicly and free of charge. mcp-fedlex does not invent any
data; it reads exclusively from there.

**Act** — the umbrella term for a single "piece" of law: a statute, an
ordinance, an international treaty. Example: the Federal Constitution is one act, the
Code of Obligations another.

**SR number** — the "shelf number" of the Systematic Compilation of federal law,
which lawyers use to cite acts. Example: SR 101 is the Federal Constitution, SR 220
the Code of Obligations. Important: SR numbers are **reused** over the
decades — a repealed act and an act in force can carry the same
number (which is why mcp-fedlex treats SR-number resolutions as discovery hints, not as
norm citations; see Chapter 4).

**ELI** — the *European Legislation Identifier*: the unique, permanent
web address of an act. Example: `eli/cc/1999/404` is the Federal Constitution.
While SR numbers can be ambiguous, an ELI is unambiguous — which is why
mcp-fedlex always works with ELIs internally and states them in every provenance record.

**Consolidated version and point-in-time date** — laws change constantly. A
*consolidated version* is the legal text with all amendments incorporated up to a
specific date — the law as it applied on that day. This date is called the
**point-in-time date**. Every answer from mcp-fedlex refers to exactly one point-in-time
date (if none is given: today).

**MCP (Model Context Protocol)** — an open standard through which AI applications
use external tools. Think of MCP as a standardized power-socket system: any
compatible AI application (e.g. Claude Desktop and many others) can connect
to an MCP server and use its tools — without any custom
adaptation.

**Tool** — a single, clearly delineated capability that the server offers to the AI,
e.g. "read article X of act Y" (`read_article`) or "search for laws
matching keyword Z" (`search_law`). mcp-fedlex offers 40 such tools
(all in Chapter 7).

**The two data sources: JOLux and AKN** — Fedlex provides its data in two forms,
and mcp-fedlex uses both for what they do best:

| | **JOLux** (metadata graph) | **AKN** (full-text documents) |
|---|---|---|
| What it is | A "knowledge network" about all acts: titles, dates, versions, who amends whom, who cites whom | The actual legal text as a structured XML document (the "Akoma Ntoso" standard) |
| Answers | "Does … exist? Since when? In which version? What is connected to what?" | "What does it say — word for word, article by article?" |
| Access | live via the SPARQL query language on the public Fedlex endpoint | the document is fetched once and then served from the cache |

Mnemonic: **JOLux never delivers legal text, AKN never delivers metadata** — the
tools combine both worlds and hide this division of labor from the AI.

## 3. How a request is processed

What happens when an AI wants to read, say, "Art. 8 of the Federal Constitution, as of
1 January 2024"? Every single call passes through the same chain — no shortcuts, no
exceptions:

```
AI application (MCP client)
      │  Call: read_article(eli, art_8), point-in-time date 2024-01-01
      ▼
┌─────────────────────────────────────────────────────────────┐
│  mcp-fedlex server ("Reader")                               │
│                                                             │
│  1. Credential check    Who is calling? Verify token.       │
│     (Auth)              No valid credential: abort.         │
│  2. Authorization       May this role even see and          │
│     (RBAC)              use this tool at all?               │
│  3. Quota               Is this session's usage budget      │
│                         not yet used up?                    │
│  4. Execute tool        Determine the version at the        │
│                         date, fetch text (cache or Fedlex). │
│  5. Provenance stamp    ELI + the effective date are        │
│     (Provenance)        stamped into the response           │
│                         SERVER-SIDE.                        │
└─────────────────────────────────────────────────────────────┘
      │  Response: norm text + provenance record
      ▼
AI application → cites: "Art. 8 BV, version of 2024-01-01"
      ▲
      └── Data source behind it: Fedlex (SPARQL endpoint + XML store)
```

Three properties of this chain are decisive:

- **The order is fixed.** First the credential, then authorization, then quota, then
  the work. An unauthenticated call never reaches a tool.
- **Fail-closed.** In every case of doubt (invalid token, unknown role, failure of the
  quota database) the server refuses — it never "waves things through when in doubt".
- **The stamp comes from the server.** The AI can *request* a point-in-time date, but
  what actually applied and was used is written into the answer by the server alone.

## 4. Norm citation or discovery hint — the centerpiece

Perhaps the most important design decision of mcp-fedlex: every answer carries one
of two **structurally distinguishable** provenance kinds.

**Norm citation (`kind: "norm"`)** — "This is the content/state of a specific, named
act at the stated point-in-time date." Examples: the text of an article
(`read_article`), the answer to "was this act in force on 1 January 2019?"
(`check_in_force`). An AI system may use norm citations as quotations.

**Discovery hint (`kind: "hint"`)** — "This could be relevant — it is a candidate, not
a quotation." Examples: results of a law search (`search_law`), the resolution of an
SR number (`resolve_sr_number`, ambiguous!), consultation documents
(legislative-history context, not law in force).

Why is this so important? An AI system doing research first finds *candidates*
("these five laws could match") and then reads the details ("this one actually
says …"). If both steps look the same, the classic mistake happens:
a search result is recorded as a substantiated norm. mcp-fedlex makes this mistake
**structurally impossible** — norm citation and discovery hint differ in the data format
itself, not just in a textual description. The correct workflow is always:
**find a discovery hint → read it with a norm-citation tool → only then cite.**

## 5. The point-in-time date — the built-in time machine

Law is time-dependent: the question "What does the law say?" is incomplete without
"… and when?". That is why the point-in-time date in mcp-fedlex is not an add-on
feature but a dimension of **every** call:

- **Every tool** accepts an optional parameter `as_of` in the format
  `YYYY-MM-DD` (e.g. `2019-01-01`). If none is given, **today** applies (Swiss time).
- The server determines the **consolidated version valid at the given date** — never
  silently "the latest".
- The **effectively used date** is always in the answer's provenance record
  (`provenance.valid_as_of`) — stamped server-side, not forgeable by any call
  parameter.
- The point-in-time date can be set in two ways: by the AI itself (in the normal
  tool call — every tool advertises the parameter) or **hard-wired by the host
  application**; the host's setting takes precedence. This way an application can,
  for example, "pin" an entire analysis to 31 December 2023 without the model being
  able to change that.
- Special case, repealed acts: if you read an act that was already repealed at the
  given date, the server still delivers the text (last version), but marks the answer
  with the repeal date (`repealed_since`) — so you see, unmistakably:
  **this is no longer law in force.**

---

# Part II — The Tools

## 6. Four toolboxes, three roles

The 40 tools are grouped into four **pools** ("toolboxes"). The pool
determines two things: *who* sees the tools (roles) and *what a call costs*
(quota, Chapter 11).

| Pool | Tools | What it can do | Data path | Cost per call |
|---|---|---|---|---|
| **LocalNavigation** | 13 | Read and navigate within the text of a known act | cache (the document is fetched once) | 1 |
| **Discovery** | 10 | **Find** acts, treaties, vocabularies — results are discovery hints | live to Fedlex | 5 |
| **JoluxMetadata** | 16 | Metadata and relationships of a **known** act — results are norm citations | live to Fedlex | 5 |
| **Validation** | 1 | Compare versions | cache | 1 |

In addition there are three **roles**, strictly nested (each higher role can do
everything the lower one can):

| Role | Sees pools | Number of tools | Typical user |
|---|---|---|---|
| **Reader** | LocalNavigation | 13 | Pure read integration |
| **Navigator** | + Discovery, JoluxMetadata | 39 | Research applications — this is how ansV runs |
| **Validator** | + Validation | 40 | Checking and comparison work |

The role is embedded in the verified access token (Chapter 10) — an AI can neither
choose nor escalate it. `tools/list` shows each role only its permitted tools,
and even someone who "guesses" another role's tool name is rejected on the call.

> In the code, two further pools (`LodFederation`, `Workspace`) are reserved for
> future tools but empty — hence the phrase "four **active**
> pools" throughout.

## 7. All 40 tools in detail

Each row: the tool name, the question it answers, and what you need to know.
("Norm citation"/"discovery hint" refers to Chapter 4.)

### Pool LocalNavigation — reading the text of an act (13 tools, norm citation — one exception)

| Tool | Answers the question | Worth knowing |
|---|---|---|
| `read_article` | "What does article X say at the given date?" | The bread-and-butter tool. For repealed acts, the answer carries the repeal date (`repealed_since`). |
| `read_element` | "What does chapter/level/element X say?" | For acts that are not structured into articles. |
| `read_document` | "Give me the whole act as readable text." | Delivers Markdown with a character budget (default 120,000) so that no AI context gets "blown up"; continuation via an offset. |
| `get_structure` | "How is the act structured?" (table of contents) | Default: a skeleton down to article level; on request the complete tree or a flat article list. |
| `search_text` | "Where in the act does term X occur?" | Searches only **within one** act; reports the total hit count and whether the list was truncated. No substitute for a semantic search. |
| `get_metadata` | "Which work, which version, which language is this — and how is it structured?" | Useful **before** `read_article`: 91% of amending acts/Federal Gazette documents have no articles at all. |
| `get_references` | "Which other acts does the text refer to?" | Paginated list of the references in the document. |
| `get_modifications` | "Which amendment instructions does this amending act contain — with the new wording?" | Empty for consolidated versions (there, amendments are already incorporated). |
| `list_components` | "Which annexes/attachments does the act have?" | Annexes are independent works with their own identity; empty annexes are marked. |
| `extract_tables` | "Which tables does the act contain?" | Tariffs, limit values, responsibility matrices — as structured units, optionally restricted to a subtree. |
| `detect_foreign_content` | "Does the act contain embedded graphics or formulas?" | Rare, but legally relevant (e.g. calculation formulas in ordinances). |
| `extract_change_notes` | "Which editorial change notes (footnotes on AS amendments) does the text carry?" | Shows the documented amendment history within the text itself. |
| `parse_unlinked_ref` | "What does the reference text 'Art. 58 Abs. 1 ParlG' mean?" | A pure text parser, hence the exception in this pool: the result is a **discovery hint** — substantiate it afterwards with `read_article`/`get_metadata`. |

### Pool Discovery — finding acts (10 tools, all discovery hints)

| Tool | Answers the question | Worth knowing |
|---|---|---|
| `search_law` | "Which acts match keyword/abbreviation/popular name X?" | Official abbreviations (OR, ZGB, DSG …) are resolved exactly and listed first; hits show whether they were in force **at the given date**; pageable. |
| `resolve_sr_number` | "Which act is behind SR number X?" | Deliberately delivers **several** candidates — SR numbers are reused; disambiguate via "in force at the given date". |
| `find_related_topic` | "Which acts belong to the same field of law?" | Deterministic navigation via the official legal taxonomy. |
| `find_treaties` | "Which international treaties exist (with country X / bilateral)?" | Filters by treaty partner and bilaterality. |
| `get_treaty_info` | "Details on this treaty process?" | Title, partners, dates, status. |
| `get_consultations` | "Which consultation procedures took place for this draft?" | Legislative-history context — by definition not law in force. |
| `get_consultation_documents` | "Which reports/opinions belong to the consultation?" | Ditto. |
| `resolve_vocabulary_label` | "What does this vocabulary URI mean in language X?" | A lookup reference for coded values from other answers. |
| `list_vocabulary` | "Which concepts does vocabulary X contain?" (e.g. the country list) | Can be filtered with a search term — e.g. "Germany" → the country URI for `find_treaties`. |
| `explore_node` | "What is attached to this node in the Fedlex data network?" | An expert tool: shows the incoming and outgoing edges of any node. |

### Pool JoluxMetadata — metadata & relationships (16 tools, all norm citations)

| Tool | Answers the question | Worth knowing |
|---|---|---|
| `check_in_force` | "Was this act in force at the given date?" | Careful, two time references: `in_force` applies **at the given date**; the vocabulary status delivered alongside is always **today's**. |
| `list_versions` | "Which versions existed — chronologically?" | Complete list; an empty list means "no consolidations", not "error". |
| `resolve_consolidation_at` | "Which version applied on day X — and where is its XML?" | The tool behind point-in-time accuracy; cleanly reports "no version at the given date" instead of delivering the latest one. |
| `get_impacts` | "Which amendments acted on this act?" | Important caveat: since 2023 Fedlex often names affected articles only in free text — an empty list does **not** prove "never amended". |
| `get_outgoing_impacts` | "Which laws does this amending act amend?" | The opposite direction of `get_impacts`; omnibus acts bundle many targets. |
| `get_article_history` | "Which amendments affected precisely this article?" | Carries the same completeness caveat directly in the answer. |
| `get_citations` | "Who cites this act — and whom does it cite?" | Only at the level of whole acts (not article-precise); directions selectable. |
| `get_taxonomy` | "Into which fields of law is the act classified?" | Around 10,000 acts are unclassified — an empty list is normal. |
| `get_subdivisions` | "Which subdivisions does the metadata graph know?" | **A catalog of gaps, not a table of contents** — the graph only knows elements with at least one amendment; the full structure comes from `get_structure`. |
| `list_annexes` | "Which annexes does the metadata graph know?" | The JOLux view (only annexes with amendments); complementary to `list_components`. |
| `get_law_metadata` | "The profile: title, abbreviation, SR number, status, dates?" | The compact business card of an act. |
| `list_expressions` | "In which languages does this version exist?" | Delivers language codes (de/fr/it/en/rm) — check before `read_article` whether e.g. Romansh exists. |
| `get_oc_act` | "Where was the act published in the Official Compilation (AS)?" | The entry point into the official publication chain. |
| `get_memorial` | "Volume/issue/pages of the AS publication?" | The classic citation locator. |
| `get_fga_documents` | "Which Federal Gazette documents (e.g. dispatches) belong to it?" | Materials for interpretation and legislative history. |
| `get_drafts` | "Which legislative drafts belong to this act?" | The entry point into the legislative history; leads on to `get_consultations`. |

### Pool Validation — comparing versions (1 tool, role Validator only)

| Tool | Answers the question | Worth knowing |
|---|---|---|
| `compare_versions` | "What changed between point-in-time date A and point-in-time date B?" | Delivers added, removed, and amended articles as a readable distillate. |

## 8. Typical workflows

Four examples of how an AI combines the tools correctly:

**"What does Art. 8 of the Federal Constitution say?"**
1. The ELI is known (`eli/cc/1999/404`) → directly `read_article` with `eid: art_8`.
2. The answer carries text + provenance record → citable.

**"Was this provision also in force on 1 January 2019?"**
1. `check_in_force` with point-in-time date `2019-01-01` → was the act in force? (norm citation)
2. `read_article` with point-in-time date `2019-01-01` → the wording at that time. (norm citation)
3. Optionally `compare_versions` (role Validator) → what has changed since then.

**"Find the authoritative law on the topic of data protection."**
1. `search_law` with "Datenschutz" (data protection) or the abbreviation "DSG" → candidates. (discovery hints!)
2. Disambiguate: which candidate was in force at the given date?
3. `get_law_metadata` + `read_article` on the chosen ELI → only now norm citations.

**"The text mentions 'Art. 58 Abs. 1 ParlG' — what is that?"**
1. `parse_unlinked_ref` decomposes the reference text → a structured candidate. (discovery hint)
2. `search_law` with the abbreviation "ParlG" → the ELI of the Parliament Act. (discovery hint)
3. `read_article`/`read_element` on the ELI → the substantiated wording. (norm citation)

---

# Part III — Trust, Security, Limits

## 9. Why the answers can be trusted

Four mechanisms interlock — all server-side, all outside the language model's
control:

1. **Provenance by construction.** Every answer carries a
   `provenance` block with the act's ELI and the effectively used point-in-time date.
   This block is generated by the server; no call parameter can set or
   overwrite it.
2. **Norm citation vs. discovery hint** (Chapter 4). Search results can structurally
   never be recorded as quotations.
3. **Identity from the credential, never from the conversation** (Chapter 10). Tenant,
   session, and role come exclusively from the verified access token. Even if a
   language model claims "I am an administrator", that changes nothing.
4. **Fail-closed as the basic stance.** Invalid credential → rejection. Unknown
   role in the token → rejection (no silent downgrade). Quota database
   down → a tight emergency quota instead of a "free ride".

Added to this is traceability over time: versioned server releases are immutable
Docker images bound to Git tags — an analysis can record
with **which server version** it was created, and the server reports it in the
protocol itself (`serverInfo.version`).

## 10. Who may do what — identity and roles

**The credential.** Every request (except pure protocol notifications) must carry a
**bearer token** in the `Authorization` header. In production this is a
**JWT** — a signed "digital ticket" from an identity provider, which the server
verifies cryptographically. The token contains the claims:

| Claim | Meaning |
|---|---|
| `iss` / `aud` / `exp` | issuer, audience, expiry — standard JWT checks |
| `tenant` | the tenant (e.g. a law firm) — pseudonymous audit identity |
| `sid` | the session — pseudonymous audit identity |
| `role` | `reader`, `navigator`, or `validator` |

A token with an unknown role is rejected. For local development there is,
as a substitute, a static dev token (never in production).

**Tenant isolation.** Everything the server enforces and logs per call
(authorization, quota, audit) is keyed to the pair `(tenant, session)` from the token.
Two organizations using the same server therefore share neither quota
nor audit trail — and neither can act in the other's name.

**Why this matters:** in classic systems, "the user" is a human.
Here, between the human and the server sits a language model that generates text — including
text like "tenant: other-law-firm". Hence the iron rule: **identity never comes
from a tool parameter**, exclusively from the verified token.

## 11. Rate limiting (quota)

The server limits how much each session may retrieve — for two reasons:
protection of the **public Fedlex endpoint** (a common good that should not be
flooded by an AI running hot) and fairness between tenants.

The model is a "token bucket" — a bucket of credit that is steadily refilled;
every call draws from it:

| Role | Bucket size (capacity) | Refill rate |
|---|---|---|
| Reader | 60 | 1 per second |
| Navigator | 120 | 2 per second |
| Validator | 240 | 4 per second |

The **cost per call** is tied to the pool, never to a parameter of the AI: reading
from the cache costs 1, live queries to Fedlex (Discovery, JoluxMetadata)
cost 5. The accounting runs through a central Redis database and thus applies
**across all server instances** — more pods do not mean more quota.
If this database fails, a tight emergency quota applies (capacity 5):
fail-closed, not fail-open.

In practice: whoever gets throttled has briefly retrieved too much — after a few
seconds the bucket refills by itself.

## 12. Data protection and logging

**What is logged.** Every tool call produces exactly one audit line:
tenant and session (pseudonymous), role, tool name, affected act (ELI),
point-in-time date, outcome status, and duration. Example:

```json
{"event":"tools/call","tool.name":"read_article","auth.role":"Navigator",
 "auth.tenant":"kanzlei-a","auth.session":"sess-1",
 "provenance.eli":"eli/cc/1999/404","provenance.valid_as_of":"2024-01-01",
 "outcome":"ok","span.duration_ms":"29"}
```

**What is never logged.** The raw call arguments (e.g. search terms that could
allow inferences about a tenant's case) and the response contents. This is
enforced by a "PII scrubber" with an **allowlist principle**: only explicitly
approved, pseudonymous fields reach the log — everything else is redacted
fail-closed. A new field is therefore *not* in the log by default, rather than
accidentally in it after all.

This keeps operations fully auditable ("who queried which act at which
date, and when"), without contents or personal data ending up in the log.

## 13. The limits of the system — honestly stated

A trustworthy system states what it **cannot** do:

- **Federal law only.** Cantonal and communal law, case law (court decisions), and
  legal literature are not included — Fedlex publishes federal law.
- **No legal advice.** mcp-fedlex delivers substantiated raw material (norm texts,
  metadata). Interpretation, weighing, and responsibility remain with humans.
- **Machine-readable full text only from about 2021 onwards.** Older consolidated
  versions often exist on Fedlex only as PDFs — the full-text tools then do not
  reach back arbitrarily far. The metadata (version list, amendments, dates) reaches
  considerably further.
- **Gaps in the metadata are normal.** The Fedlex graph guarantees hardly any field:
  around 10,000 acts are not assigned to any field of law; since 2023 the articles
  affected by amendments are often stated only in free text. The tools say so
  explicitly in their answers ("an empty list does not prove …") — an empty answer is
  a data point, not an error.
- **A third of the documents are metadata shells.** Many XML files (above all
  amending acts) have no text body; `get_metadata` detects this before you read
  into a void.
- **Dependence on Fedlex operations.** If the public Fedlex endpoint goes down,
  the Discovery/metadata tools stop working; already cached
  texts remain readable. The server then reports itself as "degraded", but
  deliberately stays online. In addition, a federal-government firewall sits in front
  of Fedlex which in rare cases blocks even legitimate queries — the server's queries
  are designed with this in mind, and a regular live conformance test serves as an
  early-warning system.
- **Searching means finding, not understanding.** `search_text` is a literal search,
  `search_law` a title/abbreviation search. A semantic search ("find things that mean
  roughly …") is deliberately not part of this server (a separate component in the
  ecosystem exists for that).

---

# Part IV — Try It Yourself

## 14. Start locally in two minutes

Prerequisite: [Docker](https://www.docker.com/) with Compose. A
Rust development environment is **not** required.

```bash
git clone <repository-url> && cd mcp-fedlex
cp .env.example .env          # defaults are fine for testing
docker compose up --build     # starts Reader + Redis
```

The server then listens on `http://localhost:8080`. Check that it is alive:

```bash
curl -s http://localhost:8080/livez    # -> "ok"
curl -s http://localhost:8080/readyz   # also checks Redis + Fedlex reachability
```

And the first real request — Article 1 of the Federal Constitution, as of 1 January
2024 (the token is the dev token from the `.env`):

```bash
TOKEN=dev-secret-change-me
curl -s -X POST http://localhost:8080/rpc \
  -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":2,"method":"tools/call",
        "params":{
          "name":"read_article",
          "arguments":{"eli":"eli/cc/1999/404","eid":"art_1"},
          "as_of":"2024-01-01"
        }
      }' | jq
```

In the answer you see both: the norm text **and** the provenance record
(`provenance` with `eli` and `valid_as_of`).

> If port 8080 is taken: `MCP_HOST_PORT=8090 docker compose up --build`
> moves only the host port; inside the container it stays 8080.

## 15. Click through in the browser (MCP Inspector)

If you prefer clicking to typing `curl`: the official **MCP Inspector** is a
browser interface for MCP servers. The repository contains a ready-made configuration
(`inspector.json`), so a single command is enough:

```bash
npx -y @modelcontextprotocol/inspector --config inspector.json --server fedlex
```

The browser opens already **connected**. The **Tools** tab lists all tools
with descriptions and input fields — try e.g. `read_article` with
`eli = eli/cc/1999/404` and `eid = art_1`.

> The bundled configuration expects the server on port 8090
> (`MCP_HOST_PORT=8090`, see Chapter 14).

## 16. Connecting an AI client

Any MCP-capable application can connect. Only two pieces of information are needed:

1. **Address:** `http://localhost:8080/mcp` (or the production URL
   `https://mcp-fedlex.ch/mcp`) — transport "Streamable HTTP".
2. **Access:** the header `Authorization: Bearer <token>`.

For the curious, the three technical routes at a glance:

| Route | Purpose |
|---|---|
| `POST /mcp` | The recommended, modern endpoint (MCP revision 2025-11-25) — with two protective checks *before* any processing (foreign browser origin → 403; unknown protocol version in the header → 400). |
| `POST /rpc` | The legacy endpoint for older clients without a handshake — same processing chain, without the two additional checks. |
| `GET /sse` | Opens an event stream (Server-Sent Events) for clients that use this older pattern. |

By default the server speaks MCP protocol revision `2025-11-25`; a
legacy client that explicitly requests `2024-11-05` still receives `2024-11-05`.
During connection setup (`initialize`) the server introduces itself with name and
version and additionally provides the model with usage instructions
("instructions") that explain, among other things, the point-in-time parameter.

## 17. Running in production — the overview

For everyone who wants to know how the system runs "properly" (details:
[`80_DEPLOY.md`](../dev/80_DEPLOY.md)):

```
Internet → Cloudflare → Ingress (nginx) → Reader pods (Kubernetes)
                                              │ encrypted (mTLS)
                                              ▼
                                        Redis (quota accounting)
```

- **Kubernetes (k3s) with GitOps:** the desired state lives versioned in Git;
  ArgoCD automatically reconciles the cluster with it.
- **Minimal containers ("distroless"):** the server image contains no shell and
  no utilities — what is not inside cannot be abused by an attacker.
- **Real credentials:** in production only JWT/JWKS (rotating keys from the
  identity provider) — never the dev token.
- **Encrypted quota database:** Redis accepts exclusively TLS with
  mutual certificates (mTLS) plus a password; secrets live encrypted in Git as
  "SealedSecrets".
- **Network closed by default:** a default-deny NetworkPolicy allows only the one
  required connection (Reader → Redis).
- **Observability:** health endpoints (`/livez`, `/readyz`, `/startupz`) and
  Prometheus metrics (calls, durations, load shedding) — metrics are reachable
  only inside the cluster.
- **Load protection:** concurrent requests are limited per instance; overload is
  rejected immediately and honestly (HTTP 503 with "Retry-After") instead of hanging
  in invisible queues; hung requests are cut off after a hard deadline
  (HTTP 504).

---

# Part V — Reference

## 18. Questions and answers (FAQ)

### General

**What is mcp-fedlex in one sentence?**
A server that gives AI systems citable, point-in-time-accurate, and verifiable
access to Swiss federal law.

**Is this a chatbot?**
No. mcp-fedlex does not itself answer questions in natural language — it is the
toolbox that a chatbot/AI agent uses to look things up correctly. The
application people talk to sits one level above it (e.g. ansV).

**Is this an official service of the federal government?**
No. mcp-fedlex is a product of the company mindful.bio. It uses exclusively the
public, official data of the federal government's Fedlex platform.

**Does it replace a lawyer?**
No. It delivers substantiated norm texts and metadata — the legal assessment
remains a human matter.

**What does it cost to use?**
The software is open source (Apache-2.0) and can be self-hosted. Access
to the hosted instance (mcp-fedlex.ch) is access-protected; it requires
a token from mindful.bio.

**Why is it called "mcp-fedlex"?**
MCP is the open standard (Model Context Protocol) through which AI applications
use tools; Fedlex is the official publication platform of federal law.
The name thus describes exactly what the server does: making Fedlex accessible via MCP.

**How are mcp-fedlex, ansV, and mindful.bio related?**
mindful.bio is the company. mcp-fedlex is its data layer (this server). ansV
(ansv.ch) is the application platform on top of it — it uses the server with the
Navigator role.

### Law and data

**Where does the data come from?**
Entirely from Fedlex: metadata live from the public SPARQL endpoint, legal texts
as AKN XML documents from the Fedlex document repository. mcp-fedlex maintains no
legal database of its own.

**How up to date are the answers?**
Metadata queries (Discovery, JoluxMetadata) go live to Fedlex — as current as
Fedlex itself. Legal texts are fetched once per version and then served from the
cache; since a consolidated version is immutable, the cache never becomes
outdated in substance.

**Does it also cover cantonal law? Court decisions?**
No, neither — Fedlex publishes federal law (acts, Official Compilation,
Federal Gazette, international treaties). Court decisions are not part of it.

**Which languages are supported?**
In principle the official languages German, French, Italian, partly English and
Romansh — depending on what Fedlex publishes for the specific version. With
`list_expressions` you can check in advance which language versions exist.

**How far back does the time machine reach?**
For metadata (versions, amendments, dates), far back. For machine-readable
full text: Fedlex provides XML versions only from about 2021 onwards; older versions
often exist only as PDFs and are out of reach for the full-text tools.

**Why does a tool deliver an empty list — is that an error?**
Usually not. The Fedlex graph guarantees hardly any field; missing information is
data ("nothing is recorded on this"), not an error. The tool answers point this out
themselves at the critical places — e.g. an empty amendment list does not prove
that an act was never amended.

**Can I read a text directly with an SR number?**
Not directly — first resolve the SR number via `resolve_sr_number` into ELI
candidates (careful, ambiguous), choose the right one, then read. Precisely because
of this ambiguity the resolution is classified as a discovery hint and not as a
norm citation.

**What about annexes and tables — are they lost?**
No. Annexes are reachable via `list_components`/`list_annexes`, tables via
`extract_tables` as structured units, and embedded formulas/graphics are reported by
`detect_foreign_content`.

### Trust and security

**Can the AI still make up answers?**
A language model can always invent text — but it cannot invent a **provenance
record**: the `provenance` block comes from the server. An application that accepts
only substantiated statements (this is how ansV works) spots unsubstantiated claims
immediately.

**Can the AI forge the point-in-time date?**
No. It can *request* a point-in-time date; what was actually used is stamped
by the server into `provenance.valid_as_of`. In addition, the host application can
pin the point-in-time date — that setting takes precedence over the model.

**Can the AI escalate its role or switch tenants?**
No. Role, tenant, and session are in the cryptographically verified token — no
tool parameter is ever read for this. A token with an unknown role is
rejected outright.

**What does "fail-closed" mean?**
When in doubt, refuse rather than let through. Examples: without a valid token there
is nothing at all; if the quota database fails, a tight emergency quota applies
instead of unlimited access; only explicitly approved fields reach the audit log.

**Are my search queries stored?**
The raw arguments (e.g. search terms) and the response contents are **not**
logged. Logged per call: pseudonymous tenant and session, role,
tool name, affected act (ELI), point-in-time date, outcome status, duration (Chapter 12).

**Can tenant A see what tenant B does?**
No. Quota and audit are keyed to the tenant/session pair from the respective
token; there is no cross-tenant access path.

**How do I report a security vulnerability?**
Confidentially to **security@mindful.bio** — please not as a public issue.
Acknowledgment of receipt usually within 3 business days (details: `SECURITY.md`).

### Technology (for the curious)

**What exactly is MCP — and which version does the server speak?**
The Model Context Protocol, an open standard for connecting tools to
AI applications, technically JSON-RPC over HTTP. The server speaks revision
`2025-11-25` (default) and `2024-11-05` (only if a legacy client explicitly
requests it).

**Which protocol methods exist?**
`initialize` (handshake and version negotiation), `tools/list` (the tools visible
to the role), `tools/call` (execute a tool), `ping` (sign of life),
plus the notification `notifications/initialized`.

**Why three HTTP routes (`/mcp`, `/rpc`, `/sse`)?**
`/mcp` is the modern, recommended endpoint with additional protective checks;
`/rpc` keeps older clients alive; `/sse` serves the older event-stream pattern.
All three lead into the same verified processing chain.

**What does the error message `-32001 missing/invalid credential` mean?**
The bearer token is missing, expired, or invalid. This is exactly how the server
is supposed to behave without a valid credential.

**Why do I get 403 or 400 at the `/mcp` endpoint?**
403: the request came with a foreign `Origin` header (protection against DNS
rebinding from the browser); permitted browser origins are maintained by the
operator via an allowlist. 400: the client announced an unsupported protocol
version in the header.

**Why 503 or 504?**
503 with "Retry-After": the instance is momentarily at full capacity and honestly
sheds the overload — wait briefly and retry. 504: a single request exceeded the hard
time limit (e.g. because an upstream is slow) and was cut off.

**Why does my client see only 13 tools?**
The token carries the Reader role. Navigator sees 39, Validator all 40 — the
tool list is filtered by role (Chapter 6).

**What is a "token bucket"?**
The quota model: a bucket of credit that is steadily refilled
(Chapter 11). It smooths load peaks without hindering normal use.

**Why is the server written in Rust?**
Memory safety without a garbage collector, strong types for the
security invariants (e.g. a verified identity is its own type that simply cannot
be constructed any other way), and predictable performance.

**Does every article retrieval run a live query to Fedlex?**
No. On first access to a version, its XML is fetched once (this includes
a short metadata query to determine which version applies at the given date); after
that, the cache serves it. Only the Discovery and metadata tools regularly go live —
which is why they cost five times as much in the quota.

**How do operations notice that something is stuck?**
Via the health endpoints (`/readyz` reports the true state including the
Redis connection; a Fedlex outage shows up as "degraded" but deliberately does not
take the server offline) and via Prometheus metrics (calls, durations, load shedding,
quota emergencies).

**Can I run the server without internet access?**
Only to a limited extent: the tools of the Discovery/JoluxMetadata pools need
the Fedlex endpoint live. The software's own tests, however, run completely
offline (live conformance is a separate, explicit test run).

### Running it yourself and contributing

**Can I run mcp-fedlex myself?**
Yes. Locally, Docker and two commands are enough (Chapter 14). For production there
are versioned, immutable Docker images
(`registry.mindful-server.com/mindful-bio/mcp-fedlex:v0.2.0`) and an
operations guide ([`80_DEPLOY.md`](../dev/80_DEPLOY.md)).

**Do I need Rust skills?**
To run it: no (Docker is enough). To contribute code: yes — start with
`CONTRIBUTING.md`.

**Where do I get a token?**
Locally: the dev token from the `.env` (full access, role Validator — for
development only). In production: from the operator's identity provider; for the
mcp-fedlex.ch instance, from mindful.bio.

**Which version is currently running?**
The server states its version during the `initialize` handshake (`serverInfo.version`);
it corresponds to the release tag. Changes are documented in the `CHANGELOG.md`.

**GitHub or GitLab — where does the project live?**
The source of truth (CI/CD, releases) is a self-hosted GitLab; GitHub
is a public mirror. Issues/PRs on GitHub are reviewed but processed
in GitLab.

**How do I cite the system in a paper or analysis?**
With the versioned release (e.g. "mcp-fedlex v0.2.0", an immutable image bound to a
Git tag) plus the provenance record of the respective answer (ELI and
point-in-time date). Together, the two make a result reproducible.

**How does this handbook become a PDF?**
With a single Pandoc command — the exact command line is in the
documentation index [`docs/README.md`](../README.md).

## 19. Glossary

| Term | Explanation |
|---|---|
| **Act** | Umbrella term for statute, ordinance, federal decree, international treaty. |
| **AKN / Akoma Ntoso** | International XML standard (OASIS) for legal documents. Fedlex publishes legal texts in this format; the full-text tools read from it. |
| **AS/OC act** | An (amending) act published in the AS; visible in ELIs as `eli/oc/…`. |
| **Audit log** | The record of all tool calls — pseudonymous and without contents (Chapter 12). |
| **Bearer token** | The "credential" of a request, sent along in the `Authorization` header. |
| **Claim** | A single, signed field in a JWT (e.g. `role`, `tenant`). |
| **Consolidated version** | Legal text with all amendments incorporated up to a given date. |
| **Consultation (procedure)** | The formal consultation procedure on legislative drafts — legislative-history context, not law in force. |
| **Discovery** | Tool pool for finding acts; results are discovery hints. |
| **Discovery hint (`hint`)** | The provenance kind "candidate, not a quotation" (Chapter 4). |
| **eId** | The stable address of an element **within** an act, e.g. `art_8` or `art_14_a`. |
| **ELI** | European Legislation Identifier — the unique, permanent identifier of an act, e.g. `eli/cc/1999/404`. |
| **Fail-closed** | The basic stance "when in doubt, refuse" — in auth, quota, and logging. |
| **Federal Gazette (BBl/FGA)** | Publication organ for dispatches, reports, and drafts — the "materials". |
| **Fedlex** | The official publication platform of Swiss federal law. |
| **FRBR** | The librarians' model "work → version → file" by which Fedlex identifies its documents. |
| **Identity provider (IdP)** | The service that authenticates users and issues JWTs. |
| **JOLux** | Fedlex's linked-data vocabulary/data model — the metadata graph over all acts. |
| **JSON-RPC** | The simple request-response format in which MCP messages are transmitted. |
| **JWT / JWKS** | JSON Web Token: a signed "digital ticket" carrying the claims. JWKS: the mechanism through which the server obtains the IdP's (rotating) verification keys. |
| **Kubernetes / Pod** | The operations platform in production; a pod is a running instance of the server. |
| **Manifestation** | FRBR term for the concrete file (e.g. the XML) of a version. |
| **MCP** | Model Context Protocol — the open standard through which AI applications use tools. |
| **mTLS** | A TLS connection with certificates verified on both sides (here: between Reader and Redis). |
| **Norm citation (`norm`)** | The provenance kind "citable statement about a named act" (Chapter 4). |
| **Official Compilation (AS)** | The official publication organ in which new federal law is promulgated (French: RO). |
| **Point-in-time date (`as_of`)** | The date to which an answer applies; default: today (Chapter 5). |
| **Pool** | A toolbox group with a shared visibility and cost rule (Chapter 6). |
| **Provenance** | The server-side-stamped provenance record of every answer (`eli`, `valid_as_of`, `kind`). |
| **Quota** | The rate limiting per tenant/session (Chapter 11). |
| **RBAC** | Role-Based Access Control — permissions are tied to the role in the token. |
| **Reader (binary)** | The server process of mcp-fedlex (the program's name; not to be confused with the Reader *role*). |
| **Redis** | The central database for quota accounting across all instances. |
| **Role** | `reader`, `navigator`, or `validator` — determines visible pools and quota. |
| **SPARQL** | Query language for linked-data graphs; used to query the JOLux metadata live. |
| **SR number** | Number in the Systematic Compilation (e.g. SR 101 = Federal Constitution); ambiguous over time. |
| **Tenant** | The organization behind a token; the basis for separating quota and audit. |
| **WAF** | Web Application Firewall — the federal government's protective filter in front of the Fedlex endpoint. |

## 20. Further documents

This handbook explains the *understanding*. The authoritative technical documents
(developer and operations documentation) are listed in the index
[`docs/README.md`](../README.md) — among them:

- **Configuration reference:** [`70_CONFIG.md`](../dev/70_CONFIG.md) — all environment variables.
- **Operations (Kubernetes):** [`80_DEPLOY.md`](../dev/80_DEPLOY.md) — topology, mTLS, runbooks.
- **Identity & roles (technical):** [`90_AUTH_AND_ROLES.md`](../dev/90_AUTH_AND_ROLES.md).
- **Architecture decisions:** [`adr/`](../dev/adr) — the reasoned course-settings (ADR-001 …).
- **Capability lexicons:** [`10_LEXICON_jolux.md`](../dev/10_LEXICON_jolux.md) and
  [`11_LEXICON_akn.md`](../dev/11_LEXICON_akn.md) — the complete capability space of the data sources.
- **Contributing:** `CONTRIBUTING.md` · **Reporting security issues:** `SECURITY.md` · **Version history:** `CHANGELOG.md`.

---

*mcp-fedlex — The Handbook · v0.2.0 · Apache-2.0 © mindful.bio ·
Project description: [mcp-fedlex.ch](https://mcp-fedlex.ch) · Application platform: [ansv.ch](https://ansv.ch)*
