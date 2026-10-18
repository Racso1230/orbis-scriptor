//! Calendar definitions and the date types built on them.
//!
//! In-world dates are stored as a plain `i64` "day number" (day 0 = first day
//! of the world's epoch). A [`CalendarDefinition`] explains how to turn that
//! number into a readable date; see `services::calendar` for the maths.

use serde::{Deserialize, Serialize};
use specta::Type;

/// One month of the year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Month {
    pub name: String,
    /// Number of days in the month (must be at least 1).
    pub days: u32,
}

/// A named span of years, e.g. "Age of Founding" (AF).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Era {
    pub name: String,
    /// Short suffix used when formatting, e.g. "AF". Falls back to `name`.
    pub abbreviation: Option<String>,
    /// Day number on which the era starts; year 1 of the era is the calendar
    /// year containing this day.
    #[specta(type = specta_typescript::Number)]
    pub start_day: i64,
}

/// Stored as JSON in `calendars.definition`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CalendarDefinition {
    pub months: Vec<Month>,
    /// Names of the days of the week; may be empty (no weekdays).
    pub weekdays: Vec<String>,
    /// Sorted by `start_day`, at least one.
    pub eras: Vec<Era>,
    /// The world's absolute day number on which this calendar's day 1 of
    /// month 1 of its year 1 falls. 0 for the default calendar. Shifting it
    /// moves how every day is *displayed*; no event ever moves.
    /// (`serde(default)` keeps definitions stored before Phase 11 loading.)
    #[serde(default)]
    #[specta(type = specta_typescript::Number)]
    pub epoch_offset: i64,
    /// Optional leap-year rule; `None` = every year has the same length.
    #[serde(default)]
    pub leap: Option<LeapRule>,
}

/// Which month gains days in leap years, and which years are leap years.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LeapRule {
    /// 0-based index of the month that gains the extra days.
    pub month_index: u32,
    /// How many days it gains in a leap year (1 to 31, usually 1).
    pub extra_days: u32,
    /// Gregorian-style cycles, listed from the smallest `every` to the
    /// largest, each `every` dividing the next one. A year uses the *last*
    /// (largest) cycle that divides its number and is leap when that cycle
    /// has `add: true`; if no cycle divides it, it is a normal year.
    pub rules: Vec<LeapCycle>,
}

/// One step of a leap rule, e.g. `{ every: 100, add: false }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LeapCycle {
    /// Years between matches (1 to 10000).
    pub every: u32,
    /// `true` = matching years are leap years; `false` = they are not.
    pub add: bool,
}

/// One problem found in a calendar definition, tied to a form field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    /// Path of the field, e.g. `name`, `months[2].days`, `leap.rules[1].every`.
    pub field: String,
    pub message: String,
}

/// Result of previewing an unsaved definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CalendarPreview {
    /// Empty when the definition is valid.
    pub issues: Vec<ValidationIssue>,
    /// One formatted string per requested day (empty when there are issues).
    pub labels: Vec<String>,
}

/// A calendar row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Calendar {
    pub id: String,
    pub name: String,
    pub definition: CalendarDefinition,
    pub is_default: bool,
}

/// A day number broken into calendar parts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CalendarDate {
    /// Index into `definition.eras`.
    pub era_index: u32,
    pub era_name: String,
    /// Year within the era; 1 is the first year. Dates before the first era
    /// continue downwards: 0, -1, -2, ...
    #[specta(type = specta_typescript::Number)]
    pub year: i64,
    /// 1-based.
    pub month: u32,
    pub month_name: String,
    /// 1-based day of the month.
    pub day: u32,
    /// Name of the weekday, or `None` when the calendar has no weekdays.
    pub weekday: Option<String>,
}

/// How [`format`](crate::services::calendar::format) writes a date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DateStyle {
    /// `15 Month 3, 1203 AF`
    Full,
    /// `15/3/1203 AF`
    Short,
    /// `Month 3, 1203 AF`
    MonthYear,
    /// `1203 AF`
    Year,
    /// `Month 3` (handy for tick labels)
    Month,
    /// `15` (handy for tick labels)
    Day,
}

/// A day number or year in command arguments (an integer that fits a JS number).
///
/// specta refuses to export `i64` (JavaScript numbers lose precision above
/// 2^53). Day numbers are limited to +/- `MAX_DAY` (10^12), far below that
/// limit, so we tell specta to describe this type as a plain TypeScript
/// `number`. `serde(transparent)` makes it travel as a bare number in JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(transparent)]
pub struct JsInt(#[specta(type = specta_typescript::Number)] pub i64);
