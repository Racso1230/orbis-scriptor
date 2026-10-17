//! Calendar maths: converting day numbers to dates and back.
//!
//! # Rules (read these before touching the code)
//!
//! * A day number is an `i64`. Day 0 is the 1st day of month 1 of the year
//!   that contains day 0. Negative numbers are days before it.
//! * Every year has the same length, `days_per_year` (the sum of the months).
//!   "Absolute year" `a = day.div_euclid(days_per_year)`: days 0..D-1 are
//!   a = 0, day -1 is a = -1, and so on. The day of the year is
//!   `day.rem_euclid(days_per_year)`, which is always 0..D-1 even for
//!   negative days (plain `/` and `%` round towards zero and would give
//!   negative remainders; `div_euclid`/`rem_euclid` always round towards
//!   minus infinity, which is what a calendar needs).
//! * Eras only change the *year number shown*. For the era that applies to a
//!   day, `year = a - era_start_absolute_year + 1`, where
//!   `era_start_absolute_year = era.start_day.div_euclid(days_per_year)`.
//!   So with a first era starting at day 0: days 0..D-1 are year 1, day -1 is
//!   year 0, day -D-1 is year -1 (astronomical numbering).
//! * The era of a day is the last era whose `start_day <= day`. Days before
//!   the first era's start still belong to the first era (with year <= 0
//!   when that era starts at day 0).
//! * Month and day-of-month always come from the day of the year, so an era
//!   starting mid-year does not shift the months.
//! * Weekday index is `day.rem_euclid(weekdays.len())`; day 0 is the first
//!   weekday.
//!
//! `to_date` and `from_date` are exact inverses for every valid day.

use rusqlite::Connection;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::calendar::{Calendar, CalendarDate, CalendarDefinition, DateStyle};

/// Day numbers beyond this are refused. It keeps all arithmetic far away from
/// `i64` overflow (and inside the exact-integer range of JavaScript numbers).
pub const MAX_DAY: i64 = 1_000_000_000_000;

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

/// Checks that a definition can be used for conversions.
pub fn validate(def: &CalendarDefinition) -> AppResult<()> {
    if def.months.is_empty() {
        return Err(AppError::Validation(
            "A calendar needs at least one month".into(),
        ));
    }
    for month in &def.months {
        if month.name.trim().is_empty() {
            return Err(AppError::Validation("Month names cannot be empty".into()));
        }
        if month.days == 0 {
            return Err(AppError::Validation(format!(
                "Month \"{}\" must have at least one day",
                month.name
            )));
        }
    }
    if def.eras.is_empty() {
        return Err(AppError::Validation(
            "A calendar needs at least one era".into(),
        ));
    }
    for era in &def.eras {
        if era.name.trim().is_empty() {
            return Err(AppError::Validation("Era names cannot be empty".into()));
        }
        check_day(era.start_day)?;
    }
    // `windows(2)` yields every pair of neighbours: [a, b], [b, c], ...
    if def
        .eras
        .windows(2)
        .any(|w| w[0].start_day >= w[1].start_day)
    {
        return Err(AppError::Validation(
            "Eras must be sorted by start day with no duplicates".into(),
        ));
    }
    Ok(())
}

/// Length of one year in days: the sum of the month lengths (at least 1 so a
/// bad definition can never cause a division by zero).
pub fn days_per_year(def: &CalendarDefinition) -> i64 {
    // `map` turns each month into its day count, `sum` adds them up.
    def.months
        .iter()
        .map(|m| i64::from(m.days))
        .sum::<i64>()
        .max(1)
}

/// The era (index) that applies to `day`.
fn era_index_for(def: &CalendarDefinition, day: i64) -> usize {
    // `rposition` searches from the end; days before every era fall back to 0.
    def.eras
        .iter()
        .rposition(|e| e.start_day <= day)
        .unwrap_or(0)
}

/// Day number -> calendar date.
pub fn to_date(def: &CalendarDefinition, day: i64) -> AppResult<CalendarDate> {
    validate(def)?;
    check_day(day)?;
    let dpy = days_per_year(def);
    let absolute_year = day.div_euclid(dpy);
    let mut day_of_year = day.rem_euclid(dpy);

    // Walk the months, subtracting each month's length until the rest fits.
    let mut month_index = 0;
    for (i, month) in def.months.iter().enumerate() {
        month_index = i;
        let len = i64::from(month.days);
        if day_of_year < len {
            break;
        }
        day_of_year -= len;
    }

    let era_index = era_index_for(def, day);
    let era = &def.eras[era_index];
    let year = absolute_year - era.start_day.div_euclid(dpy) + 1;
    let weekday = if def.weekdays.is_empty() {
        None
    } else {
        // `as usize` is safe: rem_euclid by a positive length is in range.
        let len = def.weekdays.len() as i64;
        Some(def.weekdays[day.rem_euclid(len) as usize].clone())
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

/// Calendar parts -> day number. `era_index` + `year` identify the year;
/// `month` and `day` are 1-based and must exist in the calendar.
pub fn from_date(
    def: &CalendarDefinition,
    era_index: u32,
    year: i64,
    month: u32,
    day: u32,
) -> AppResult<i64> {
    validate(def)?;
    let era = def
        .eras
        .get(era_index as usize)
        .ok_or_else(|| AppError::Validation(format!("Era {era_index} does not exist")))?;
    let month_pos = (month as usize)
        .checked_sub(1)
        .filter(|&i| i < def.months.len())
        .ok_or_else(|| AppError::Validation(format!("Month {month} does not exist")))?;
    let month_def = &def.months[month_pos];
    if day == 0 || day > month_def.days {
        return Err(AppError::Validation(format!(
            "{} has {} days; day {day} does not exist",
            month_def.name, month_def.days
        )));
    }

    let dpy = days_per_year(def);
    // Days from the start of the year to the start of this month.
    let before_month: i64 = def.months[..month_pos]
        .iter()
        .map(|m| i64::from(m.days))
        .sum();
    // Inverse of the year formula in `to_date`. Checked maths so absurd
    // years return an error instead of overflowing.
    let absolute_year = year
        .checked_sub(1)
        .and_then(|y| y.checked_add(era.start_day.div_euclid(dpy)))
        .ok_or_else(|| AppError::Validation("Year is out of range".into()))?;
    let result = absolute_year
        .checked_mul(dpy)
        .and_then(|d| d.checked_add(before_month + i64::from(day) - 1))
        .ok_or_else(|| AppError::Validation("Year is out of range".into()))?;
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
    use crate::models::calendar::{Era, Month};

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
}
