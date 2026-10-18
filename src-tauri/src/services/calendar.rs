//! Calendar maths: converting day numbers to dates and back.
//!
//! # Rules (read these before touching the code)
//!
//! * A day number is an `i64`. All calendars of a world share the same
//!   *absolute* day numbers; a calendar is only a *view* that decides how a
//!   day number is written. Converting between calendars therefore means
//!   formatting the same day number with another calendar.
//! * `epoch_offset` (E) is the absolute day number on which the calendar's
//!   "year index 0" begins. The calendar-local day is `local = day - E`.
//! * Year index `a` (an `i64`) counts calendar years from that epoch: local
//!   days of year 0 are the first year, year -1 is the one before, and so on.
//!   The *astronomical year number* used for leap tests is `Y = a + 1`
//!   (so index 0 is year 1, index -1 is year 0, index -2 is year -1).
//! * Year length: `base` (the sum of the month lengths) plus `extra_days`
//!   when `Y` is a leap year. The extra days are added to the month
//!   `leap.month_index`. Without a leap rule every year has `base` days.
//! * Leap test: the rules are nested cycles ordered by `every` (each divides
//!   the next). The *largest* `every` that divides `Y` decides; `add: true`
//!   means leap. Gregorian = `[4 add, 100 remove, 400 add]`: 1900 is not
//!   leap, 2000 and 2024 are. `rem_euclid` makes this work for negative
//!   years too (plain `%` can return negative remainders).
//! * Days before year `a` (from the start of index 0): `a * base +
//!   extra * F(a)` where `F(n)` counts leap years among `Y = 1..=n` (negative
//!   for negative `n`). `F` uses floor division (`div_euclid`) per cycle,
//!   so there is no loop over years and huge values stay fast. The inverse
//!   (which year holds a day) is a binary search on that increasing function.
//! * Eras only change the *year number shown*. For the era that applies to a
//!   day, `year = a - era_year_index + 1`, where `era_year_index` is the year
//!   index containing `era.start_day`. The era of a day is the last era with
//!   `start_day <= day` (absolute day); days before the first era still
//!   belong to the first era (years 0, -1, ...). Month and day-of-month come
//!   from the position inside the year, so an era starting mid-year does not
//!   shift the months. (Eras that count years backwards are not supported.)
//! * Weekday index is `(day - E).rem_euclid(weekdays.len())`; weekdays carry
//!   on unbroken across leap days.
//!
//! With `epoch_offset = 0` and no leap rule every result is identical to the
//! pre-Phase-11 behaviour. `to_date` and `from_date` are exact inverses.

use rusqlite::Connection;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::calendar::{
    Calendar, CalendarDate, CalendarDefinition, DateStyle, LeapCycle, LeapRule, ValidationIssue,
};

/// Day numbers beyond this are refused. It keeps all arithmetic far away from
/// `i64` overflow (and inside the exact-integer range of JavaScript numbers).
pub const MAX_DAY: i64 = 1_000_000_000_000;

/// Limits enforced by [`validate_definition`].
pub const MAX_MONTHS: usize = 100;
pub const MAX_MONTH_DAYS: u32 = 1000;
pub const MAX_WEEKDAYS: usize = 30;
pub const MAX_NAME_LEN: usize = 60;
pub const MAX_ABBREVIATION_LEN: usize = 10;
pub const MAX_LEAP_RULES: usize = 8;
pub const MAX_LEAP_EVERY: u32 = 10_000;
pub const MAX_EXTRA_DAYS: u32 = 31;

/// Checks a day number is inside +/- [`MAX_DAY`].
pub fn check_day(day: i64) -> AppResult<()> {
    if (-MAX_DAY..=MAX_DAY).contains(&day) {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "Day number {day} is out of range"
        )))
    }
}

fn issue(field: impl Into<String>, message: impl Into<String>) -> ValidationIssue {
    ValidationIssue {
        field: field.into(),
        message: message.into(),
    }
}

/// Every problem in a definition, with the field it belongs to. An empty
/// list means the definition is valid. (`Vec::new()` does not allocate, so
/// calling this on every conversion is cheap for valid definitions.)
pub fn validate_definition(def: &CalendarDefinition) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    if def.months.is_empty() {
        issues.push(issue("months", "A calendar needs at least one month"));
    }
    if def.months.len() > MAX_MONTHS {
        issues.push(issue(
            "months",
            format!("A calendar can have at most {MAX_MONTHS} months"),
        ));
    }
    for (i, month) in def.months.iter().enumerate() {
        check_name(&mut issues, &format!("months[{i}].name"), &month.name);
        if month.days == 0 || month.days > MAX_MONTH_DAYS {
            issues.push(issue(
                format!("months[{i}].days"),
                format!("A month needs 1 to {MAX_MONTH_DAYS} days"),
            ));
        }
    }

    if def.weekdays.len() > MAX_WEEKDAYS {
        issues.push(issue(
            "weekdays",
            format!("A calendar can have at most {MAX_WEEKDAYS} weekdays"),
        ));
    }
    for (i, name) in def.weekdays.iter().enumerate() {
        check_name(&mut issues, &format!("weekdays[{i}]"), name);
    }

    if def.eras.is_empty() {
        issues.push(issue("eras", "A calendar needs at least one era"));
    }
    for (i, era) in def.eras.iter().enumerate() {
        check_name(&mut issues, &format!("eras[{i}].name"), &era.name);
        if let Some(abbr) = &era.abbreviation
            && abbr.trim().chars().count() > MAX_ABBREVIATION_LEN
        {
            issues.push(issue(
                format!("eras[{i}].abbreviation"),
                format!("Abbreviations can have at most {MAX_ABBREVIATION_LEN} characters"),
            ));
        }
        if check_day(era.start_day).is_err() {
            issues.push(issue(
                format!("eras[{i}].startDay"),
                "The start day is out of range",
            ));
        }
    }
    // `windows(2)` yields every pair of neighbours: [a, b], [b, c], ...
    for (i, w) in def.eras.windows(2).enumerate() {
        if w[0].start_day >= w[1].start_day {
            issues.push(issue(
                format!("eras[{}].startDay", i + 1),
                "Eras must be sorted by start day with no duplicates",
            ));
        }
    }

    if check_day(def.epoch_offset).is_err() {
        issues.push(issue("epochOffset", "The epoch offset is out of range"));
    }

    if let Some(leap) = &def.leap {
        validate_leap(def, leap, &mut issues);
    }
    issues
}

fn validate_leap(def: &CalendarDefinition, leap: &LeapRule, issues: &mut Vec<ValidationIssue>) {
    if leap.month_index as usize >= def.months.len() {
        issues.push(issue(
            "leap.monthIndex",
            "The leap month must be one of the calendar's months",
        ));
    }
    if leap.extra_days == 0 || leap.extra_days > MAX_EXTRA_DAYS {
        issues.push(issue(
            "leap.extraDays",
            format!("A leap year adds 1 to {MAX_EXTRA_DAYS} days"),
        ));
    }
    if leap.rules.is_empty() || leap.rules.len() > MAX_LEAP_RULES {
        issues.push(issue(
            "leap.rules",
            format!("A leap rule needs 1 to {MAX_LEAP_RULES} cycles"),
        ));
    }
    for (i, rule) in leap.rules.iter().enumerate() {
        if rule.every == 0 || rule.every > MAX_LEAP_EVERY {
            issues.push(issue(
                format!("leap.rules[{i}].every"),
                format!("Cycles are 1 to {MAX_LEAP_EVERY} years long"),
            ));
        }
    }
    // Nesting: each cycle length must be a larger multiple of the previous
    // one (so lengths are also distinct and ascending). The `!= 0` guard
    // avoids a divide-by-zero panic on bad input.
    for (i, w) in leap.rules.windows(2).enumerate() {
        let nested = w[0].every != 0 && w[1].every > w[0].every && w[1].every % w[0].every == 0;
        if !nested {
            issues.push(issue(
                format!("leap.rules[{}].every", i + 1),
                "List cycles from smallest to largest; each must be a larger multiple of the one before (e.g. 4, 100, 400)",
            ));
        }
    }
}

fn check_name(issues: &mut Vec<ValidationIssue>, field: &str, name: &str) {
    let len = name.trim().chars().count();
    if len == 0 {
        issues.push(issue(field, "The name cannot be empty"));
    } else if len > MAX_NAME_LEN {
        issues.push(issue(
            field,
            format!("Names can have at most {MAX_NAME_LEN} characters"),
        ));
    }
}

/// Checks that a definition can be used for conversions (first problem only).
pub fn validate(def: &CalendarDefinition) -> AppResult<()> {
    match validate_definition(def).into_iter().next() {
        None => Ok(()),
        Some(first) => Err(AppError::Validation(first.message)),
    }
}

/// Length of a normal (non-leap) year in days: the sum of the month lengths
/// (at least 1 so a bad definition can never cause a division by zero).
pub fn days_per_year(def: &CalendarDefinition) -> i64 {
    // `map` turns each month into its day count, `sum` adds them up.
    def.months
        .iter()
        .map(|m| i64::from(m.days))
        .sum::<i64>()
        .max(1)
}

// ---- Leap years ----

/// Is astronomical year number `y` a leap year under these cycles?
/// The largest cycle that divides `y` decides.
fn is_leap_year_number(rules: &[LeapCycle], y: i64) -> bool {
    rules
        .iter()
        .rev()
        .find(|r| r.every != 0 && y.rem_euclid(i64::from(r.every)) == 0)
        .is_some_and(|r| r.add)
}

/// `F(n)`: how many leap years there are among year numbers `1..=n`
/// (for negative `n`: minus the leap years among `n+1..=0`).
///
/// A year is decided by the largest cycle dividing it, so cycle `i` decides
/// the years divisible by `every_i` but not by `every_{i+1}`. Because the
/// cycles are nested those are `floor(n/every_i) - floor(n/every_{i+1})`
/// years. `div_euclid` is floor division, which also behaves for negatives.
fn leap_count(rules: &[LeapCycle], n: i64) -> i64 {
    let mut total = 0;
    for (i, rule) in rules.iter().enumerate() {
        if !rule.add || rule.every == 0 {
            continue;
        }
        let mut count = n.div_euclid(i64::from(rule.every));
        if let Some(next) = rules.get(i + 1)
            && next.every != 0
        {
            count -= n.div_euclid(i64::from(next.every));
        }
        total += count;
    }
    total
}

/// Is year index `a` (see module docs) a leap year in this calendar?
pub fn is_leap_year(def: &CalendarDefinition, year_index: i64) -> bool {
    match &def.leap {
        Some(leap) => is_leap_year_number(&leap.rules, year_index + 1),
        None => false,
    }
}

/// Number of days in year index `a`.
pub fn days_in_year(def: &CalendarDefinition, year_index: i64) -> i64 {
    let extra = match &def.leap {
        Some(leap) if is_leap_year(def, year_index) => i64::from(leap.extra_days),
        _ => 0,
    };
    days_per_year(def) + extra
}

/// Number of days in month `month_index` (0-based) of year index `a`.
fn month_length(def: &CalendarDefinition, year_index: i64, month_index: usize) -> i64 {
    let mut len = i64::from(def.months[month_index].days);
    if let Some(leap) = &def.leap
        && leap.month_index as usize == month_index
        && is_leap_year(def, year_index)
    {
        len += i64::from(leap.extra_days);
    }
    len
}

/// Local days from the start of year index 0 to the start of year index `a`
/// (negative when `a` is negative). `base` is `days_per_year(def)`.
fn days_before_year(def: &CalendarDefinition, base: i64, year_index: i64) -> i64 {
    let leaps = match &def.leap {
        Some(leap) => i64::from(leap.extra_days) * leap_count(&leap.rules, year_index),
        None => 0,
    };
    year_index * base + leaps
}

/// The year index that contains local day `local`.
fn year_index_of(def: &CalendarDefinition, base: i64, local: i64) -> i64 {
    let extra = def.leap.as_ref().map_or(0, |l| i64::from(l.extra_days));
    // Find the largest `a` with days_before_year(a) <= local, by binary
    // search (the function strictly increases because every year has at
    // least one day). `lo` always satisfies the condition, `hi` never does.
    // Years are between `base` and `base + extra` days long, which gives
    // safe starting bounds on each side of zero.
    let (mut lo, mut hi) = if local >= 0 {
        (local.div_euclid(base + extra), local.div_euclid(base) + 1)
    } else {
        (local.div_euclid(base), 0)
    };
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if days_before_year(def, base, mid) <= local {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// The era (index) that applies to `day`.
fn era_index_for(def: &CalendarDefinition, day: i64) -> usize {
    // `rposition` searches from the end; days before every era fall back to 0.
    def.eras
        .iter()
        .rposition(|e| e.start_day <= day)
        .unwrap_or(0)
}

/// Year index in which an era starts (so that year is the era's year 1).
fn era_start_index(def: &CalendarDefinition, base: i64, era_index: usize) -> i64 {
    year_index_of(def, base, def.eras[era_index].start_day - def.epoch_offset)
}

/// The year index containing absolute `day`, and that year's length. Handy
/// for tests and the shared fixtures.
pub fn year_info(def: &CalendarDefinition, day: i64) -> AppResult<(i64, i64)> {
    validate(def)?;
    check_day(day)?;
    let base = days_per_year(def);
    let index = year_index_of(def, base, day - def.epoch_offset);
    Ok((index, days_in_year(def, index)))
}

/// Day number -> calendar date.
pub fn to_date(def: &CalendarDefinition, day: i64) -> AppResult<CalendarDate> {
    validate(def)?;
    check_day(day)?;
    let base = days_per_year(def);
    let local = day - def.epoch_offset;
    let year_index = year_index_of(def, base, local);
    let mut day_of_year = local - days_before_year(def, base, year_index);

    // Walk the months, subtracting each month's length until the rest fits.
    let mut month_index = 0;
    for i in 0..def.months.len() {
        month_index = i;
        let len = month_length(def, year_index, i);
        if day_of_year < len {
            break;
        }
        day_of_year -= len;
    }

    let era_index = era_index_for(def, day);
    let era = &def.eras[era_index];
    let year = year_index - era_start_index(def, base, era_index) + 1;
    let weekday = if def.weekdays.is_empty() {
        None
    } else {
        // `as usize` is safe: rem_euclid by a positive length is in range.
        let len = def.weekdays.len() as i64;
        Some(def.weekdays[local.rem_euclid(len) as usize].clone())
    };

    Ok(CalendarDate {
        era_index: era_index as u32,
        era_name: era.name.clone(),
        year,
        month: month_index as u32 + 1,
        month_name: def.months[month_index].name.clone(),
        day: day_of_year as u32 + 1,
        weekday,
    })
}

/// Resolves (era, era-relative year) to a year index, rejecting absurd years.
fn year_index_from_era(
    def: &CalendarDefinition,
    base: i64,
    era_index: u32,
    year: i64,
) -> AppResult<i64> {
    if def.eras.get(era_index as usize).is_none() {
        return Err(AppError::Validation(format!(
            "Era {era_index} does not exist"
        )));
    }
    let start = era_start_index(def, base, era_index as usize);
    // Years beyond this would exceed MAX_DAY anyway; rejecting them first
    // keeps every multiplication below far from overflow.
    year.checked_sub(1)
        .and_then(|y| y.checked_add(start))
        .filter(|i| i.abs() <= 2 * MAX_DAY)
        .ok_or_else(|| AppError::Validation("Year is out of range".into()))
}

/// Number of days in a month (1-based `month`) of the given era year; this
/// is what a date editor needs to fill its day dropdown (leap years differ).
pub fn days_in_month(
    def: &CalendarDefinition,
    era_index: u32,
    year: i64,
    month: u32,
) -> AppResult<u32> {
    validate(def)?;
    let base = days_per_year(def);
    let year_index = year_index_from_era(def, base, era_index, year)?;
    let month_pos = (month as usize)
        .checked_sub(1)
        .filter(|&i| i < def.months.len())
        .ok_or_else(|| AppError::Validation(format!("Month {month} does not exist")))?;
    Ok(month_length(def, year_index, month_pos) as u32)
}

/// Calendar parts -> day number. `era_index` + `year` identify the year;
/// `month` and `day` are 1-based and must exist in the calendar (a leap day
/// only exists in leap years).
pub fn from_date(
    def: &CalendarDefinition,
    era_index: u32,
    year: i64,
    month: u32,
    day: u32,
) -> AppResult<i64> {
    validate(def)?;
    let base = days_per_year(def);
    let year_index = year_index_from_era(def, base, era_index, year)?;
    let month_pos = (month as usize)
        .checked_sub(1)
        .filter(|&i| i < def.months.len())
        .ok_or_else(|| AppError::Validation(format!("Month {month} does not exist")))?;
    let month_len = month_length(def, year_index, month_pos);
    if day == 0 || i64::from(day) > month_len {
        return Err(AppError::Validation(format!(
            "{} has {} days in that year; day {day} does not exist",
            def.months[month_pos].name, month_len
        )));
    }
    // Days from the start of the year to the start of this month.
    let before_month: i64 = (0..month_pos)
        .map(|i| month_length(def, year_index, i))
        .sum();
    let result =
        def.epoch_offset + days_before_year(def, base, year_index) + before_month + i64::from(day)
            - 1;
    check_day(result)?;
    Ok(result)
}

fn era_label(def: &CalendarDefinition, date: &CalendarDate) -> String {
    let era = &def.eras[date.era_index as usize];
    era.abbreviation
        .clone()
        .filter(|a| !a.trim().is_empty())
        .unwrap_or_else(|| era.name.clone())
}

/// Formats a day number. Examples with the default calendar:
/// `Full` "15 Month 3, 1203 AF", `Short` "15/3/1203 AF",
/// `MonthYear` "Month 3, 1203 AF", `Year` "1203 AF", `Month` "Month 3",
/// `Day` "15". Years <= 0 print as-is, e.g. "0 AF" or "-5 AF".
pub fn format(def: &CalendarDefinition, day: i64, style: DateStyle) -> AppResult<String> {
    let d = to_date(def, day)?;
    let era = era_label(def, &d);
    Ok(match style {
        DateStyle::Full => format!("{} {}, {} {}", d.day, d.month_name, d.year, era),
        DateStyle::Short => format!("{}/{}/{} {}", d.day, d.month, d.year, era),
        DateStyle::MonthYear => format!("{}, {} {}", d.month_name, d.year, era),
        DateStyle::Year => format!("{} {}", d.year, era),
        DateStyle::Month => d.month_name,
        DateStyle::Day => d.day.to_string(),
    })
}

/// Reads a date written the way [`format`] writes it, so dates in an
/// exported note can be turned back into day numbers: `Full`
/// ("15 Month 3, 1203 AF") or `Short` ("15/3/1203 AF"). The era label may be
/// its abbreviation or its name (any case); without one the first era is
/// used. Returns `None` for anything that is not a valid date of `def`.
pub fn parse_date(def: &CalendarDefinition, text: &str) -> Option<i64> {
    let text = text.trim();
    let (first, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let (day, month, year, era_text): (u32, u32, i64, &str) = if first.contains('/') {
        // Short: d/m/y then the era label.
        let mut parts = first.split('/');
        let day = parts.next()?.parse().ok()?;
        let month = parts.next()?.parse().ok()?;
        let year = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        (day, month, year, rest.trim())
    } else {
        // Full: day, month name, comma, year, era label.
        let day: u32 = first.parse().ok()?;
        let rest = rest.trim_start();
        let lower = rest.to_lowercase();
        // The longest month name that starts the text ("Month 12" before "Month 1").
        let (month_pos, month_len) = def
            .months
            .iter()
            .enumerate()
            .filter(|(_, m)| lower.starts_with(&m.name.to_lowercase()))
            .map(|(i, m)| (i, m.name.len()))
            .max_by_key(|(_, len)| *len)?;
        let after = rest.get(month_len..)?.trim_start_matches([',', ' ']);
        let (year_text, era_text) = after.split_once(char::is_whitespace).unwrap_or((after, ""));
        (
            day,
            month_pos as u32 + 1,
            year_text.parse().ok()?,
            era_text.trim(),
        )
    };
    let era_index = if era_text.is_empty() {
        0
    } else {
        let wanted = era_text.to_lowercase();
        def.eras.iter().position(|e| {
            e.name.to_lowercase() == wanted
                || e.abbreviation
                    .as_deref()
                    .is_some_and(|a| !a.is_empty() && a.to_lowercase() == wanted)
        })? as u32
    };
    from_date(def, era_index, year, month, day).ok()
}

// ---- Service functions that load calendars from the database ----

pub fn get_default(conn: &Connection) -> AppResult<Calendar> {
    db::calendars::get_default(conn)?
        .ok_or_else(|| AppError::Internal("This world has no default calendar".into()))
}

pub fn list(conn: &Connection) -> AppResult<Vec<Calendar>> {
    db::calendars::list(conn)
}

/// The named calendar, or the default one when `calendar_id` is `None`.
pub fn resolve(conn: &Connection, calendar_id: Option<&str>) -> AppResult<Calendar> {
    match calendar_id {
        None => get_default(conn),
        Some(id) => db::calendars::get(conn, id)?
            .ok_or_else(|| AppError::NotFound(format!("Calendar {id} does not exist"))),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::models::calendar::{Era, LeapCycle, LeapRule, Month};

    fn month(name: &str, days: u32) -> Month {
        Month {
            name: name.into(),
            days,
        }
    }

    /// 12 x 30 days, one era from day 0, 7 weekdays (like migration 007).
    fn standard() -> CalendarDefinition {
        CalendarDefinition {
            months: (1..=12).map(|i| month(&format!("Month {i}"), 30)).collect(),
            weekdays: ["A", "B", "C", "D", "E", "F", "G"]
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            eras: vec![Era {
                name: "Age of Founding".into(),
                abbreviation: Some("AF".into()),
                start_day: 0,
            }],
            epoch_offset: 0,
            leap: None,
        }
    }

    /// Uneven months (60-day year), no weekdays, two eras.
    fn odd() -> CalendarDefinition {
        CalendarDefinition {
            months: vec![month("Frost", 20), month("Bloom", 33), month("Ember", 7)],
            weekdays: vec![],
            eras: vec![
                Era {
                    name: "First".into(),
                    abbreviation: None,
                    start_day: -500,
                },
                Era {
                    name: "Second".into(),
                    abbreviation: Some("SE".into()),
                    start_day: 1000,
                },
            ],
            epoch_offset: 0,
            leap: None,
        }
    }

    #[test]
    fn year_zero_and_negative_years() {
        let def = standard();
        let parts = |day| {
            let d = to_date(&def, day).unwrap();
            (d.year, d.month, d.day)
        };
        assert_eq!(parts(0), (1, 1, 1));
        assert_eq!(parts(359), (1, 12, 30));
        assert_eq!(parts(360), (2, 1, 1));
        assert_eq!(parts(-1), (0, 12, 30));
        assert_eq!(parts(-360), (0, 1, 1));
        assert_eq!(parts(-361), (-1, 12, 30));
    }

    #[test]
    fn month_boundaries() {
        let def = standard();
        let d = to_date(&def, 29).unwrap();
        assert_eq!((d.month, d.day), (1, 30));
        let d = to_date(&def, 30).unwrap();
        assert_eq!((d.month, d.day, d.month_name.as_str()), (2, 1, "Month 2"));
    }

    #[test]
    fn weekdays_wrap_for_negative_days() {
        let def = standard();
        assert_eq!(to_date(&def, 0).unwrap().weekday.as_deref(), Some("A"));
        assert_eq!(to_date(&def, 7).unwrap().weekday.as_deref(), Some("A"));
        assert_eq!(to_date(&def, -1).unwrap().weekday.as_deref(), Some("G"));
        assert!(to_date(&odd(), 5).unwrap().weekday.is_none());
    }

    fn round_trip(def: &CalendarDefinition, day: i64) {
        let d = to_date(def, day).unwrap();
        let back = from_date(def, d.era_index, d.year, d.month, d.day).unwrap();
        assert_eq!(back, day, "round trip failed for {day} ({d:?})");
    }

    #[test]
    fn round_trips_every_day_in_a_wide_range() {
        for def in [standard(), odd()] {
            for day in -100_000..=100_000 {
                round_trip(&def, day);
            }
        }
    }

    #[test]
    fn round_trips_extreme_values() {
        for def in [standard(), odd()] {
            for day in [
                -MAX_DAY,
                -MAX_DAY + 1,
                MAX_DAY - 1,
                MAX_DAY,
                123_456_789_012,
            ] {
                round_trip(&def, day);
            }
        }
    }

    #[test]
    fn eras_switch_on_their_start_day() {
        // Year length 60. -500 div_euclid 60 = -9, so era "First" begins in
        // absolute year -9 (its year 1).
        let def = odd();
        let before = to_date(&def, -600).unwrap(); // absolute year -10
        assert_eq!((before.era_index, before.year), (0, 0));
        let first = to_date(&def, -500).unwrap();
        assert_eq!((first.era_index, first.year), (0, 1));
        // Day 999 is still the first era: absolute year 16 -> year 26.
        let last_first = to_date(&def, 999).unwrap();
        assert_eq!((last_first.era_index, last_first.year), (0, 26));
        // Day 1000 starts "Second" (mid-year); 1000 div_euclid 60 = 16 is
        // year 1 of the new era, and the month still follows the day of year.
        let second = to_date(&def, 1000).unwrap();
        assert_eq!((second.era_index, second.era_name.as_str()), (1, "Second"));
        assert_eq!((second.year, second.month), (1, 2)); // day-of-year 40 -> Bloom
        assert_eq!(to_date(&def, 1020).unwrap().year, 2);
    }

    #[test]
    fn formats_dates() {
        let def = standard();
        let day = from_date(&def, 0, 1203, 3, 15).unwrap();
        assert_eq!(
            format(&def, day, DateStyle::Full).unwrap(),
            "15 Month 3, 1203 AF"
        );
        assert_eq!(format(&def, day, DateStyle::Short).unwrap(), "15/3/1203 AF");
        assert_eq!(
            format(&def, day, DateStyle::MonthYear).unwrap(),
            "Month 3, 1203 AF"
        );
        assert_eq!(format(&def, day, DateStyle::Year).unwrap(), "1203 AF");
        assert_eq!(format(&def, day, DateStyle::Month).unwrap(), "Month 3");
        assert_eq!(format(&def, day, DateStyle::Day).unwrap(), "15");
        assert_eq!(format(&def, -1, DateStyle::Year).unwrap(), "0 AF");
        assert_eq!(format(&def, -361, DateStyle::Year).unwrap(), "-1 AF");
        // An era without an abbreviation falls back to its name.
        assert_eq!(format(&odd(), 0, DateStyle::Year).unwrap(), "10 First");
    }

    #[test]
    fn from_date_rejects_impossible_dates() {
        let def = standard();
        assert!(from_date(&def, 0, 1, 13, 1).is_err());
        assert!(from_date(&def, 0, 1, 0, 1).is_err());
        assert!(from_date(&def, 0, 1, 1, 0).is_err());
        assert!(from_date(&def, 0, 1, 1, 31).is_err());
        assert!(from_date(&def, 5, 1, 1, 1).is_err());
        assert!(from_date(&def, 0, i64::MAX, 1, 1).is_err());
    }

    #[test]
    fn validation_catches_bad_definitions() {
        let mut def = standard();
        def.months.clear();
        assert!(validate(&def).is_err());

        let mut def = standard();
        def.months[0].days = 0;
        assert!(validate(&def).is_err());

        let mut def = standard();
        def.eras.clear();
        assert!(validate(&def).is_err());

        let mut def = odd();
        def.eras.reverse();
        assert!(validate(&def).is_err());

        let mut def = odd();
        def.eras[1].start_day = def.eras[0].start_day;
        assert!(validate(&def).is_err());

        assert!(validate(&standard()).is_ok());
        assert!(to_date(&standard(), MAX_DAY + 1).is_err());
    }

    #[test]
    fn days_per_year_sums_months() {
        assert_eq!(days_per_year(&standard()), 360);
        assert_eq!(days_per_year(&odd()), 60);
    }

    // ---- Phase 11: leap rules and epoch offset ----

    fn cycle(every: u32, add: bool) -> LeapCycle {
        LeapCycle { every, add }
    }

    fn gregorian_rules() -> Vec<LeapCycle> {
        vec![cycle(4, true), cycle(100, false), cycle(400, true)]
    }

    /// Real-world month lengths with a leap day at the end of February. The
    /// era starts at the epoch, so the year shown equals the leap-test year.
    fn gregorian(epoch_offset: i64) -> CalendarDefinition {
        let lengths = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        CalendarDefinition {
            months: lengths
                .iter()
                .enumerate()
                .map(|(i, d)| month(&format!("M{}", i + 1), *d))
                .collect(),
            weekdays: ["A", "B", "C", "D", "E", "F", "G"]
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            eras: vec![Era {
                name: "Common".into(),
                abbreviation: Some("CE".into()),
                start_day: epoch_offset,
            }],
            epoch_offset,
            leap: Some(LeapRule {
                month_index: 1,
                extra_days: 1,
                rules: gregorian_rules(),
            }),
        }
    }

    /// A few odd rule sets for the brute-force comparisons.
    fn rule_sets() -> Vec<Vec<LeapCycle>> {
        vec![
            gregorian_rules(),
            vec![cycle(4, true)],
            vec![cycle(3, true), cycle(9, false), cycle(27, true)],
            vec![cycle(1, true), cycle(7, false)],
            vec![cycle(5, false), cycle(10, true)],
        ]
    }

    fn with_rules(rules: Vec<LeapCycle>) -> CalendarDefinition {
        let mut def = odd(); // 60-day year, 3 months
        def.leap = Some(LeapRule {
            month_index: 1,
            extra_days: 3,
            rules,
        });
        def
    }

    #[test]
    fn leap_year_predicate_matches_gregorian() {
        let rules = gregorian_rules();
        for (year, leap) in [
            (1900, false),
            (2000, true),
            (2024, true),
            (2023, false),
            (2100, false),
            (0, true),
            (-4, true),
            (-100, false),
            (-400, true),
            (-1, false),
        ] {
            assert_eq!(is_leap_year_number(&rules, year), leap, "year {year}");
        }
        // Year index = year number - 1.
        assert!(is_leap_year(&gregorian(0), 1999));
        assert!(!is_leap_year(&standard(), 3));
    }

    #[test]
    fn days_before_year_matches_a_brute_force_loop() {
        for rules in rule_sets() {
            let def = with_rules(rules);
            let base = days_per_year(&def);
            // Walk forwards from index 0 ...
            let mut total = 0;
            for a in 0..=2000 {
                assert_eq!(days_before_year(&def, base, a), total, "a = {a}");
                total += days_in_year(&def, a);
            }
            // ... and backwards.
            let mut total = 0;
            for a in (-2000..=0).rev() {
                assert_eq!(days_before_year(&def, base, a), total, "a = {a}");
                total -= days_in_year(&def, a - 1);
            }
        }
    }

    #[test]
    fn gregorian_month_lengths_and_leap_days() {
        let def = gregorian(0);
        // Year 2024 is leap: February has 29 days; 2023 and 1900 do not.
        assert_eq!(days_in_month(&def, 0, 2024, 2).unwrap(), 29);
        assert_eq!(days_in_month(&def, 0, 2023, 2).unwrap(), 28);
        assert_eq!(days_in_month(&def, 0, 1900, 2).unwrap(), 28);
        assert_eq!(days_in_month(&def, 0, 2000, 2).unwrap(), 29);
        assert_eq!(days_in_month(&def, 0, 2024, 3).unwrap(), 31);
        assert!(days_in_month(&def, 0, 2024, 13).is_err());

        let leap_day = from_date(&def, 0, 2024, 2, 29).unwrap();
        let next = to_date(&def, leap_day + 1).unwrap();
        assert_eq!((next.month, next.day), (3, 1));
        assert!(from_date(&def, 0, 2023, 2, 29).is_err());
        assert!(from_date(&def, 0, 1900, 2, 29).is_err());
        // 366 days in a leap year, 365 otherwise.
        let jan1 = from_date(&def, 0, 2024, 1, 1).unwrap();
        let next_jan1 = from_date(&def, 0, 2025, 1, 1).unwrap();
        assert_eq!(next_jan1 - jan1, 366);
        assert_eq!(
            from_date(&def, 0, 2023, 1, 1).unwrap() + 365,
            jan1,
            "2023 has 365 days"
        );
    }

    #[test]
    fn round_trips_with_leap_rules() {
        for rules in rule_sets() {
            let def = with_rules(rules);
            for day in -30_000..=30_000 {
                round_trip(&def, day);
            }
        }
        let def = gregorian(12_345);
        for day in -200_000..=200_000 {
            round_trip(&def, day);
        }
    }

    #[test]
    fn round_trips_extreme_values_with_leap_rules() {
        // One-day months with a 31-day leap month: years are 1 or 32 days
        // long, the hardest case for the year search bounds.
        let tiny = CalendarDefinition {
            months: vec![month("Only", 1)],
            weekdays: vec![],
            eras: vec![Era {
                name: "E".into(),
                abbreviation: None,
                start_day: 0,
            }],
            epoch_offset: -777,
            leap: Some(LeapRule {
                month_index: 0,
                extra_days: 31,
                rules: vec![cycle(3, true)],
            }),
        };
        for def in [
            gregorian(0),
            gregorian(-5_000_000),
            with_rules(gregorian_rules()),
            tiny,
        ] {
            for day in [
                -MAX_DAY,
                -MAX_DAY + 1,
                -999_999_999,
                -1_000_000_007,
                999_999_937,
                MAX_DAY - 1,
                MAX_DAY,
                123_456_789_012,
                -987_654_321_098,
            ] {
                round_trip(&def, day);
            }
        }
        // Dense check around a huge value too.
        let def = gregorian(0);
        for day in 999_999_000_000..999_999_002_000 {
            round_trip(&def, day);
        }
    }

    #[test]
    fn epoch_offset_shifts_the_view_not_the_days() {
        // Same shape, shifted by 100 days: day 100 is day 1 of year 1.
        let plain = standard();
        let mut shifted = standard();
        shifted.epoch_offset = 100;
        shifted.eras[0].start_day = 100;
        let at = |def: &CalendarDefinition, day| {
            let d = to_date(def, day).unwrap();
            (d.year, d.month, d.day)
        };
        assert_eq!(at(&shifted, 100), (1, 1, 1));
        assert_eq!(at(&shifted, 99), (0, 12, 30));
        assert_eq!(at(&shifted, 100 + 360), (2, 1, 1));
        assert_eq!(at(&plain, 100), (1, 4, 11));
        // Weekdays follow the offset: the epoch day is the first weekday.
        assert_eq!(
            to_date(&shifted, 100).unwrap().weekday.as_deref(),
            Some("A")
        );
        assert_eq!(to_date(&shifted, 99).unwrap().weekday.as_deref(), Some("G"));
        // The same absolute day lies in different years in the two views.
        let day = from_date(&shifted, 0, 5, 12, 3).unwrap();
        assert_eq!(day, 100 + 4 * 360 + 330 + 2);
        assert_ne!(at(&plain, day).0, 5);
    }

    #[test]
    fn eras_use_the_calendar_own_year_index() {
        // Epoch 1000, era 2 starts mid-year at day 1000 + 360 * 3 + 40.
        let mut def = standard();
        def.epoch_offset = 1000;
        def.eras = vec![
            Era {
                name: "One".into(),
                abbreviation: None,
                start_day: 1000,
            },
            Era {
                name: "Two".into(),
                abbreviation: None,
                start_day: 1000 + 360 * 3 + 40,
            },
        ];
        let d = to_date(&def, 1000 + 360 * 3 + 40).unwrap();
        assert_eq!((d.era_index, d.year, d.month), (1, 1, 2));
        assert_eq!(to_date(&def, 1000 + 360 * 4).unwrap().year, 2);
        round_trip(&def, 1000 + 360 * 3 + 40);
    }

    #[test]
    fn weekdays_continue_across_leap_days() {
        let def = gregorian(0);
        let leap_day = from_date(&def, 0, 2024, 2, 29).unwrap();
        let names = ["A", "B", "C", "D", "E", "F", "G"];
        let index = |day| {
            let w = to_date(&def, day).unwrap().weekday.unwrap();
            names.iter().position(|n| *n == w).unwrap()
        };
        for day in leap_day - 10..leap_day + 10 {
            assert_eq!((index(day) + 1) % 7, index(day + 1));
        }
    }

    #[test]
    fn leap_cycles_can_remove_and_nest_deeply() {
        // [every 2 add, every 6 remove, every 12 add]: Y%12==0 leap,
        // Y%6==0 not, other even years leap.
        let rules = vec![cycle(2, true), cycle(6, false), cycle(12, true)];
        assert!(is_leap_year_number(&rules, 4));
        assert!(!is_leap_year_number(&rules, 6));
        assert!(!is_leap_year_number(&rules, 18));
        assert!(is_leap_year_number(&rules, 12));
        assert!(!is_leap_year_number(&rules, 5));
        let def = with_rules(rules);
        for day in -5_000..5_000 {
            round_trip(&def, day);
        }
    }

    fn fields(def: &CalendarDefinition) -> Vec<String> {
        validate_definition(def)
            .into_iter()
            .map(|i| i.field)
            .collect()
    }

    #[test]
    fn validation_reports_field_level_issues() {
        assert!(validate_definition(&standard()).is_empty());
        assert!(validate_definition(&gregorian(0)).is_empty());

        let mut def = standard();
        def.months[2].days = 0;
        def.months[3].name = "  ".into();
        def.months[4].days = MAX_MONTH_DAYS + 1;
        def.eras[0].abbreviation = Some("TOOLONGABBREV".into());
        def.epoch_offset = MAX_DAY + 1;
        def.weekdays = (0..31).map(|i| format!("W{i}")).collect();
        let f = fields(&def);
        for expected in [
            "months[2].days",
            "months[3].name",
            "months[4].days",
            "eras[0].abbreviation",
            "epochOffset",
            "weekdays",
        ] {
            assert!(f.iter().any(|x| x == expected), "{expected} in {f:?}");
        }

        let mut def = standard();
        def.months = (0..101).map(|i| month(&format!("m{i}"), 1)).collect();
        def.weekdays.clear(); // no weekdays is allowed
        assert_eq!(fields(&def), vec!["months"]);

        let mut def = standard();
        def.eras[0].name = "x".repeat(61);
        assert_eq!(fields(&def), vec!["eras[0].name"]);
    }

    #[test]
    fn validation_checks_leap_rules() {
        let leap = |month_index, extra_days, rules| {
            let mut def = standard();
            def.leap = Some(LeapRule {
                month_index,
                extra_days,
                rules,
            });
            def
        };
        assert!(validate_definition(&leap(11, 31, gregorian_rules())).is_empty());
        assert_eq!(
            fields(&leap(12, 1, gregorian_rules())),
            vec!["leap.monthIndex"]
        );
        assert_eq!(
            fields(&leap(0, 0, gregorian_rules())),
            vec!["leap.extraDays"]
        );
        assert_eq!(
            fields(&leap(0, 32, gregorian_rules())),
            vec!["leap.extraDays"]
        );
        assert_eq!(fields(&leap(0, 1, vec![])), vec!["leap.rules"]);
        assert_eq!(
            fields(&leap(0, 1, vec![cycle(1, true); 9]))
                .first()
                .map(String::as_str),
            Some("leap.rules")
        );
        assert_eq!(
            fields(&leap(0, 1, vec![cycle(0, true)])),
            vec!["leap.rules[0].every"]
        );
        assert_eq!(
            fields(&leap(0, 1, vec![cycle(10_001, true)])),
            vec!["leap.rules[0].every"]
        );
        // Duplicate, descending and non-nested cycles are refused.
        for rules in [
            vec![cycle(4, true), cycle(4, false)],
            vec![cycle(100, false), cycle(4, true)],
            vec![cycle(4, true), cycle(6, true)],
        ] {
            assert_eq!(fields(&leap(0, 1, rules)), vec!["leap.rules[1].every"]);
        }
        assert!(to_date(&leap(0, 0, gregorian_rules()), 0).is_err());
    }

    #[test]
    fn stored_definitions_without_phase_11_fields_still_load() {
        let json = r#"{"months":[{"name":"A","days":30}],"weekdays":[],"eras":[{"name":"E","abbreviation":null,"startDay":0}]}"#;
        let def: CalendarDefinition = serde_json::from_str(json).unwrap();
        assert_eq!(def.epoch_offset, 0);
        assert!(def.leap.is_none());
    }

    // ---- Shared golden fixtures for the TypeScript port ----

    /// 13 months of 28 days, a leap day every 4 years except every 100, two
    /// eras (one before the epoch), no weekdays, shifted epoch.
    fn moons() -> CalendarDefinition {
        CalendarDefinition {
            months: (1..=13).map(|i| month(&format!("Moon {i}"), 28)).collect(),
            weekdays: vec![],
            eras: vec![
                Era {
                    name: "Before the Reckoning".into(),
                    abbreviation: Some("BR".into()),
                    start_day: -100_000,
                },
                Era {
                    name: "Reckoning".into(),
                    abbreviation: None,
                    start_day: -250,
                },
            ],
            epoch_offset: -250,
            leap: Some(LeapRule {
                month_index: 12,
                extra_days: 1,
                rules: vec![cycle(4, true), cycle(100, false)],
            }),
        }
    }

    const STYLES: [(&str, DateStyle); 6] = [
        ("full", DateStyle::Full),
        ("short", DateStyle::Short),
        ("monthYear", DateStyle::MonthYear),
        ("year", DateStyle::Year),
        ("month", DateStyle::Month),
        ("day", DateStyle::Day),
    ];

    /// Days worth checking: fixed spot values plus, for a range of years,
    /// the days around year starts, ends and the leap month.
    fn fixture_days(def: &CalendarDefinition) -> Vec<i64> {
        let mut days: Vec<i64> = vec![
            0,
            1,
            -1,
            29,
            30,
            359,
            360,
            -360,
            -361,
            365,
            366,
            -365,
            -366,
            1_000,
            -1_000,
            99_999,
            -99_999,
            1_000_000,
            -1_000_000,
            1_000_000_000,
            -1_000_000_000,
            1_234_567_891,
            -1_234_567_891,
            999_999_999_999,
            -999_999_999_999,
            MAX_DAY,
            -MAX_DAY,
        ];
        for year in [
            -401, -400, -100, -5, -4, -1, 0, 1, 2, 3, 4, 5, 99, 100, 101, 399, 400, 401, 2023,
            2024, 2100,
        ] {
            let Ok(start) = from_date(def, 0, year, 1, 1) else {
                continue;
            };
            let (_, len) = year_info(def, start).unwrap();
            days.extend([start - 1, start, start + 1, start + len - 1, start + len]);
            if let Some(leap) = &def.leap {
                let m = leap.month_index + 1;
                let m_len = days_in_month(def, 0, year, m).unwrap();
                if let Ok(last) = from_date(def, 0, year, m, m_len) {
                    days.extend([last - 1, last, last + 1]);
                }
            }
        }
        days.retain(|d| check_day(*d).is_ok());
        days.sort_unstable();
        days.dedup();
        days
    }

    fn fixtures_json() -> String {
        let calendars = [
            ("default", standard()),
            ("gregorian", gregorian(1_000)),
            ("moons", moons()),
            ("uneven", odd()),
        ];
        let mut out = String::from("{\n  \"calendars\": [\n");
        for (ci, (name, def)) in calendars.iter().enumerate() {
            out.push_str(&format!(
                "    {{\n      \"name\": {},\n      \"definition\": {},\n      \"samples\": [\n",
                serde_json::to_string(name).unwrap(),
                serde_json::to_string(def).unwrap()
            ));
            let days = fixture_days(def);
            for (di, &day) in days.iter().enumerate() {
                let date = to_date(def, day).unwrap();
                let mut formats = serde_json::Map::new();
                for (key, style) in STYLES {
                    formats.insert(key.into(), format(def, day, style).unwrap().into());
                }
                let sample = serde_json::json!({
                    "day": day,
                    "date": date,
                    "formats": formats,
                    "daysInYear": year_info(def, day).unwrap().1,
                    "daysInMonth": days_in_month(def, date.era_index, date.year, date.month).unwrap(),
                });
                out.push_str("        ");
                out.push_str(&serde_json::to_string(&sample).unwrap());
                out.push_str(if di + 1 < days.len() { ",\n" } else { "\n" });
            }
            out.push_str("      ]\n    }");
            out.push_str(if ci + 1 < calendars.len() {
                ",\n"
            } else {
                "\n"
            });
        }
        out.push_str("  ]\n}\n");
        out
    }

    #[test]
    fn parse_date_reads_what_format_writes() {
        for def in [standard(), gregorian(0), moons()] {
            for day in (-200_000..200_000)
                .step_by(997)
                .chain([-250, -251, 0, 59, 60])
            {
                for style in [DateStyle::Full, DateStyle::Short] {
                    let text = format(&def, day, style).unwrap();
                    assert_eq!(parse_date(&def, &text), Some(day), "{text}");
                }
            }
        }
    }

    #[test]
    fn parse_date_is_forgiving_but_strict_about_nonsense() {
        let def = standard();
        let day = from_date(&def, 0, 1203, 3, 15).unwrap();
        for text in [
            "15 Month 3, 1203 AF",
            "  15 month 3, 1203 af ",
            "15 Month 3 1203 Age of Founding",
            "15 Month 3, 1203",
            "15/3/1203 AF",
        ] {
            assert_eq!(parse_date(&def, text), Some(day), "{text}");
        }
        // "Month 12" must not be read as "Month 1" followed by "2".
        let twelve = from_date(&def, 0, 5, 12, 1).unwrap();
        assert_eq!(parse_date(&def, "1 Month 12, 5 AF"), Some(twelve));
        for bad in [
            "",
            "soon",
            "2024-05-17",
            "31 Month 3, 1203 AF",
            "15 Nowhere, 1203 AF",
            "15 Month 3, 1203 XX",
            "0 Month 3, 1203 AF",
            "1/2/3/4 AF",
        ] {
            assert_eq!(parse_date(&def, bad), None, "{bad:?}");
        }
    }

    /// Regenerates `src/features/timeline/calendar-fixtures.json` (like the
    /// bindings export). The TypeScript port must reproduce every value in
    /// it. When the `CI` environment variable is set the file is not
    /// rewritten; instead the test fails if the committed file is stale.
    #[test]
    fn shared_fixtures_file_is_in_sync() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../src/features/timeline/calendar-fixtures.json");
        let fresh = fixtures_json();
        let on_disk = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        if on_disk != fresh {
            assert!(
                std::env::var_os("CI").is_none(),
                "calendar-fixtures.json is out of date; run `cargo test` locally and commit it"
            );
            std::fs::write(&path, &fresh).unwrap();
        }
    }
}
