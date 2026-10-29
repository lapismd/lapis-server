use super::{DailyFormatContext, macros};
use chrono::{Datelike, NaiveDate};

fn number(value: i64, width: usize) -> String {
    if value < 0 {
        format!("-{:0width$}", value.unsigned_abs())
    } else {
        format!("{value:0width$}")
    }
}
fn last_two(value: i32) -> i64 {
    let text = value.to_string();
    text[text.len().saturating_sub(2)..]
        .parse()
        .expect("year suffix")
}
fn local_week(date: NaiveDate, context: &DailyFormatContext) -> (i32, i32) {
    let local = |date: NaiveDate| {
        (date.weekday().number_from_monday() as i32 - context.first_day as i32 + 7) % 7 + 1
    };
    let offset = |year| {
        -local(NaiveDate::from_ymd_opt(year, 1, context.minimal_days).unwrap())
            + context.minimal_days as i32
            - 1
    };
    let weeks = |year| {
        (NaiveDate::from_ymd_opt(year, 12, 31).unwrap().ordinal() as i32 - offset(year)
            + offset(year + 1))
            / 7
    };
    let week =
        (date.ordinal() as i32 - local(date) + 14 - context.minimal_days as i32).div_euclid(7);
    if week < 1 {
        (date.year() - 1, weeks(date.year() - 1))
    } else if week > weeks(date.year()) {
        (date.year() + 1, 1)
    } else {
        (date.year(), week)
    }
}
pub(super) fn render(date: NaiveDate, token: &str, context: &DailyFormatContext) -> String {
    let month = macros::MONTHS[date.month0() as usize];
    let day = macros::DAYS[date.weekday().num_days_from_monday() as usize];
    let iso = date.iso_week();
    let local = local_week(date, context);
    let before_christ = context.locale == "en-GB" && date.year() <= 0;
    match token {
        "S" | "s" | "m" | "H" | "uuu" => "0".into(),
        "u" | "SSS" => "000".into(),
        "uu" | "ss" | "mm" | "HH" => "00".into(),
        "h" | "hh" => "12".into(),
        "a" => if context.locale == "en-GB" {
            "am"
        } else {
            "AM"
        }
        .into(),
        "Z" => "+0".into(),
        "ZZ" => "+00:00".into(),
        "ZZZ" => "+0000".into(),
        "ZZZZ" | "ZZZZZ" | "z" => "UTC".into(),
        "d" => date.day().to_string(),
        "dd" => number(date.day() as i64, 2),
        "c" | "E" => date.weekday().number_from_monday().to_string(),
        "ccc" | "EEE" => day[..3].into(),
        "cccc" | "EEEE" => day.into(),
        "ccccc" | "EEEEE" => day[..1].into(),
        "M" | "L" => date.month().to_string(),
        "MM" | "LL" => number(date.month() as i64, 2),
        "MMM" | "LLL" => month[..3].into(),
        "MMMM" | "LLLL" => month.into(),
        "MMMMM" | "LLLLL" => month[..1].into(),
        "y" => date.year().to_string(),
        "yy" => number(last_two(date.year()), 2),
        "yyyy" => number(date.year() as i64, 4),
        "yyyyyy" => number(date.year() as i64, 6),
        "G" => if before_christ { "BC" } else { "AD" }.into(),
        "GG" => if before_christ {
            "Before Christ"
        } else {
            "Anno Domini"
        }
        .into(),
        "GGGGG" => if before_christ { "B" } else { "A" }.into(),
        "kk" => number(last_two(iso.year()), 2),
        "kkkk" => number(iso.year() as i64, 4),
        "W" => iso.week().to_string(),
        "WW" => number(iso.week() as i64, 2),
        "n" => local.1.to_string(),
        "nn" => number(local.1 as i64, 2),
        "ii" => number(last_two(local.0), 2),
        "iiii" => number(local.0 as i64, 4),
        "o" => date.ordinal().to_string(),
        "ooo" => number(date.ordinal() as i64, 3),
        "q" => ((date.month() - 1) / 3 + 1).to_string(),
        "qq" => number(((date.month() - 1) / 3 + 1) as i64, 2),
        "X" => date
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp()
            .to_string(),
        "x" => date
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis()
            .to_string(),
        _ => macros::render(date, token, context).unwrap_or_else(|| token.into()),
    }
}
