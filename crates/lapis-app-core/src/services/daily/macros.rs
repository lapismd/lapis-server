use super::DailyFormatContext;
use chrono::{Datelike, NaiveDate};

pub(super) const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
pub(super) const DAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// Captured English Gregorian Intl macros at the UTC-midnight source boundary.
pub(super) fn render(date: NaiveDate, token: &str, context: &DailyFormatContext) -> Option<String> {
    let month = MONTHS[date.month0() as usize];
    let weekday = DAYS[date.weekday().num_days_from_monday() as usize];
    let year = if date.year() <= 0 {
        1 - date.year()
    } else {
        date.year()
    };
    let british = context.locale == "en-GB";
    let (short, medium, full) = if british {
        (
            format!("{:02}/{:02}/{year}", date.day(), date.month()),
            format!("{} {} {year}", date.day(), &month[..3]),
            format!("{} {month} {year}", date.day()),
        )
    } else {
        (
            format!("{}/{}/{year}", date.month(), date.day()),
            format!("{} {}, {year}", &month[..3], date.day()),
            format!("{month} {}, {year}", date.day()),
        )
    };
    let huge = format!("{weekday}, {full}");
    let time = if british { "00:00" } else { "12:00\u{202f}AM" };
    let seconds = if british {
        "00:00:00"
    } else {
        "12:00:00\u{202f}AM"
    };
    let zoned_seconds = if british {
        "0:00:00 UTC".into()
    } else {
        format!("{seconds} UTC")
    };
    Some(match token {
        "D" => short,
        "DD" => medium,
        "DDD" => full,
        "DDDD" => huge,
        "t" => time.into(),
        "tt" => seconds.into(),
        "ttt" | "tttt" => zoned_seconds,
        "T" => "00:00".into(),
        "TT" => "00:00:00".into(),
        "TTT" | "TTTT" => {
            if british {
                zoned_seconds
            } else {
                "00:00:00 UTC".into()
            }
        }
        "f" => format!("{short}, {time}"),
        "ff" => format!("{medium}, {time}"),
        "fff" => format!("{full} at {time} UTC"),
        "ffff" => format!("{huge} at {time} UTC"),
        "F" => format!("{short}, {seconds}"),
        "FF" => format!("{medium}, {seconds}"),
        "FFF" => format!("{full} at {zoned_seconds}"),
        "FFFF" => format!("{huge} at {zoned_seconds}"),
        _ => return None,
    })
}
