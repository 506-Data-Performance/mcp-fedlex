//! Temporal Resolver (Point-in-Time, ADR-004-nah).
//!
//! Juristisch zwingend. Jede Abfrage bezieht sich auf einen Stichtag, nicht
//! blind auf die neueste Fassung. Der Resolver stempelt eine Anfrage mit
//! [`ValidAsOf`] (welche Fassung galt) und [`TransactionTime`] (wann erfasst).
//! Aus diesem Stempel und der aufgelösten Quelle entsteht später die
//! [`Provenance`] der Antwort. Damit hängen Anfrage-Stempel und Antwort-Herkunft
//! an denselben zwei Zeitachsen.

use fedlex_core::{Eli, Provenance, TransactionTime, ValidAsOf};
use time::{Date, Month, OffsetDateTime, UtcOffset, Weekday};

/// Stempel einer einzelnen Anfrage. Bindet beide Zeitachsen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryStamp {
    valid_as_of: ValidAsOf,
    transaction_time: TransactionTime,
}

impl QueryStamp {
    /// Gültigkeitszeit der Anfrage.
    pub fn valid_as_of(&self) -> ValidAsOf {
        self.valid_as_of
    }

    /// Systemzeit der Anfrage.
    pub fn transaction_time(&self) -> TransactionTime {
        self.transaction_time
    }

    /// Leitet aus diesem Stempel und der aufgelösten Quelle die Antwort-Herkunft
    /// ab. So trägt jede Antwort exakt den Stichtag, gegen den gefragt wurde.
    pub fn into_provenance(self, eli: Eli) -> Provenance {
        Provenance::new(eli, self.valid_as_of, self.transaction_time)
    }

    /// Leitet aus diesem Stempel und einem Treffer-ELI eine **Hinweis**-Herkunft
    /// ab (Discovery, ADR-006). Sie sagt „zum Stichtag X als Kandidat gefunden",
    /// nicht „Norm Y besagt Z" — der Konsument darf sie nicht als Beleg zählen.
    pub fn into_hint_provenance(self, eli: Eli) -> Provenance {
        Provenance::hint(eli, self.valid_as_of, self.transaction_time)
    }
}

/// Stempelt Anfragen mit dem juristischen Stichtag.
#[derive(Debug, Clone, Copy)]
pub struct TemporalResolver {
    /// Fester Default-Stichtag (deterministische Tests). `None` = der heutige
    /// Kalendertag in der Schweiz, **zum Zeitpunkt der Anfrage** berechnet.
    /// Ein beim Prozessstart eingefrorenes «heute» veraltet mit jedem Lauftag
    /// des Pods und lieferte real den Vortag (Agent-UX-Befund, 68 §F-6).
    default_as_of: Option<Date>,
}

impl TemporalResolver {
    /// Erzeugt einen Resolver mit explizitem, festem Default-Stichtag
    /// (für deterministische Tests).
    pub fn new(default_as_of: Date) -> Self {
        Self {
            default_as_of: Some(default_as_of),
        }
    }

    /// Erzeugt den Produktions-Resolver: Default-Stichtag ist der heutige
    /// Kalendertag in der Schweiz (Europe/Zurich), je Anfrage neu bestimmt.
    pub fn swiss_today() -> Self {
        Self {
            default_as_of: None,
        }
    }

    /// Stempelt eine Anfrage. Gibt der Agent einen Stichtag an, gilt dieser,
    /// sonst der Default. Die Systemzeit ist immer der reale Erfassungszeitpunkt.
    pub fn stamp(&self, requested_as_of: Option<Date>) -> QueryStamp {
        self.stamp_at(requested_as_of, TransactionTime::now())
    }

    /// Wie [`Self::stamp`], aber mit fester Systemzeit (für deterministische
    /// Tests). Der «heute»-Default leitet sich aus genau dieser Systemzeit ab —
    /// Stichtag und Erfassungszeitpunkt hängen damit an derselben Uhr.
    pub fn stamp_at(&self, requested_as_of: Option<Date>, tx: TransactionTime) -> QueryStamp {
        let default = self
            .default_as_of
            .unwrap_or_else(|| swiss_date_at(tx.instant()));
        QueryStamp {
            valid_as_of: ValidAsOf::new(requested_as_of.unwrap_or(default)),
            transaction_time: tx,
        }
    }
}

/// Kalendertag in der Schweiz (Europe/Zurich) zum gegebenen Zeitpunkt.
///
/// Schweizer Bundesrecht tritt um Mitternacht **Schweizer Zeit** in Kraft;
/// der UTC-Kalendertag hinkt dem zwischen 22:00/23:00 UTC und Mitternacht
/// hinterher und wäre als Default-Stichtag an Grenztagen falsch. CET/CEST
/// wird direkt gerechnet statt über eine tz-Datenbank: Die Regel (Sommerzeit
/// vom letzten März-Sonntag 01:00 UTC bis zum letzten Oktober-Sonntag
/// 01:00 UTC) ist seit 1996 gesetzlich fixiert und EU/CH-identisch.
pub fn swiss_date_at(instant: OffsetDateTime) -> Date {
    let utc = instant.to_offset(UtcOffset::UTC);
    let year = utc.year();
    let dst_start = last_sunday_utc_1am(year, Month::March);
    let dst_end = last_sunday_utc_1am(year, Month::October);
    let offset = if utc >= dst_start && utc < dst_end {
        UtcOffset::from_hms(2, 0, 0).expect("CEST ist ein gueltiger Offset")
    } else {
        UtcOffset::from_hms(1, 0, 0).expect("CET ist ein gueltiger Offset")
    };
    utc.to_offset(offset).date()
}

/// 01:00 UTC am letzten Sonntag des Monats (März/Oktober haben 31 Tage).
fn last_sunday_utc_1am(year: i32, month: Month) -> OffsetDateTime {
    let mut d = Date::from_calendar_date(year, month, 31).expect("Maerz und Oktober haben 31 Tage");
    while d.weekday() != Weekday::Sunday {
        d = d.previous_day().expect("Monat enthaelt einen Sonntag");
    }
    d.with_hms(1, 0, 0)
        .expect("01:00 ist eine gueltige Uhrzeit")
        .assume_utc()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::{date, datetime};

    #[test]
    fn uses_default_when_no_stichtag_given() {
        let r = TemporalResolver::new(date!(2024 - 01 - 01));
        let stamp = r.stamp_at(None, TransactionTime::new(datetime!(2026-06-01 09:00 UTC)));
        assert_eq!(stamp.valid_as_of().to_string(), "2024-01-01");
    }

    // 68 §F-6: Der Produktions-Default ist der Anfrage-Tag, nicht der
    // Boot-Tag — abgeleitet aus derselben Uhr wie transaction_time.
    #[test]
    fn swiss_today_derives_default_from_transaction_time() {
        let r = TemporalResolver::swiss_today();
        let stamp = r.stamp_at(None, TransactionTime::new(datetime!(2026-07-06 09:00 UTC)));
        assert_eq!(stamp.valid_as_of().to_string(), "2026-07-06");
    }

    // Grenztag: 22:30 UTC ist in der Schweiz (CEST, +2) schon der Folgetag.
    // Ein UTC-Default hätte hier den juristisch falschen Vortag gestempelt.
    #[test]
    fn swiss_today_crosses_midnight_before_utc() {
        let r = TemporalResolver::swiss_today();
        let stamp = r.stamp_at(None, TransactionTime::new(datetime!(2026-07-05 22:30 UTC)));
        assert_eq!(stamp.valid_as_of().to_string(), "2026-07-06");
    }

    #[test]
    fn swiss_date_summer_and_winter_offsets() {
        // Sommer (CEST, +2): 22:30 UTC -> Folgetag; Winter (CET, +1): noch derselbe Tag.
        assert_eq!(
            swiss_date_at(datetime!(2026-04-10 22:30 UTC)).to_string(),
            "2026-04-11"
        );
        assert_eq!(
            swiss_date_at(datetime!(2026-11-10 22:30 UTC)).to_string(),
            "2026-11-10"
        );
        // Winter kippt erst um 23:00 UTC auf den Folgetag.
        assert_eq!(
            swiss_date_at(datetime!(2026-11-10 23:30 UTC)).to_string(),
            "2026-11-11"
        );
    }

    #[test]
    fn swiss_date_dst_boundaries_2026() {
        // Letzter Maerz-Sonntag 2026 ist der 29., letzter Oktober-Sonntag der 25.
        // Unmittelbar vor der Umstellung gilt CET, ab 01:00 UTC CEST (und umgekehrt).
        assert_eq!(
            swiss_date_at(datetime!(2026-03-29 00:30 UTC)).to_string(),
            "2026-03-29"
        );
        assert_eq!(
            swiss_date_at(datetime!(2026-10-24 22:30 UTC)).to_string(),
            "2026-10-25" // noch CEST: +2 kippt den Tag
        );
        assert_eq!(
            swiss_date_at(datetime!(2026-10-25 22:30 UTC)).to_string(),
            "2026-10-25" // wieder CET: +1 reicht nicht bis Mitternacht
        );
    }

    // Ein expliziter Stichtag schlaegt den «heute»-Default auch im
    // Produktions-Modus.
    #[test]
    fn swiss_today_honours_requested_stichtag() {
        let r = TemporalResolver::swiss_today();
        let stamp = r.stamp_at(
            Some(date!(2019 - 07 - 01)),
            TransactionTime::new(datetime!(2026-07-06 09:00 UTC)),
        );
        assert_eq!(stamp.valid_as_of().to_string(), "2019-07-01");
    }

    #[test]
    fn honours_requested_stichtag() {
        let r = TemporalResolver::new(date!(2024 - 01 - 01));
        let stamp = r.stamp_at(
            Some(date!(2019 - 07 - 01)),
            TransactionTime::new(datetime!(2026-06-01 09:00 UTC)),
        );
        assert_eq!(stamp.valid_as_of().to_string(), "2019-07-01");
    }

    #[test]
    fn stamp_carries_into_provenance() {
        let r = TemporalResolver::new(date!(2024 - 01 - 01));
        let stamp = r.stamp_at(
            Some(date!(2020 - 01 - 01)),
            TransactionTime::new(datetime!(2026-06-01 09:00 UTC)),
        );
        let prov = stamp.into_provenance(Eli::new("eli/cc/1999/404").unwrap());
        assert_eq!(prov.eli.as_str(), "eli/cc/1999/404");
        assert_eq!(prov.valid_as_of.to_string(), "2020-01-01");
    }

    // ADR-006: derselbe Stempel erzeugt für Discovery eine Hinweis-Herkunft,
    // die sich strukturell von der Norm-Herkunft unterscheidet.
    #[test]
    fn stamp_carries_into_hint_provenance() {
        let r = TemporalResolver::new(date!(2024 - 01 - 01));
        let stamp = r.stamp_at(
            Some(date!(2020 - 01 - 01)),
            TransactionTime::new(datetime!(2026-06-01 09:00 UTC)),
        );
        let eli = Eli::new("eli/cc/2017/762").unwrap();
        let hint = stamp.into_hint_provenance(eli.clone());
        assert_eq!(hint.eli.as_str(), "eli/cc/2017/762");
        assert!(
            !hint.is_norm(),
            "Discovery-Treffer ist ein Hinweis, kein Beleg"
        );
        // Norm- und Hinweis-Herkunft aus demselben Stempel sind nicht gleich.
        assert_ne!(stamp.into_provenance(eli), hint);
    }
}
