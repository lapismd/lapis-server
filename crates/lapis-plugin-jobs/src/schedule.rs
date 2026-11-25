use chrono::{DateTime, Duration, LocalResult, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema, ts_rs::TS)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "camelCase")]
pub enum Schedule {
    Interval {
        seconds: u32,
    },
    Daily {
        hour: u8,
        minute: u8,
        timezone: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Occurrence {
    pub key: String,
    pub scheduled_at: i64,
    pub local_date: String,
}

impl Schedule {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Interval { seconds } if (60..=2_678_400).contains(seconds) => Ok(()),
            Self::Daily {
                hour,
                minute,
                timezone,
            } if *hour < 24 && *minute < 60 && timezone.parse::<Tz>().is_ok() => Ok(()),
            _ => Err("Choose an interval from one minute to 31 days or a valid daily time".into()),
        }
    }

    /// Collapse missed occurrences to the latest due slot. UTC milliseconds.
    pub fn latest(&self, now: i64) -> Result<Occurrence, String> {
        self.validate()?;
        let now = DateTime::from_timestamp_millis(now).ok_or("Invalid job time")?;
        match self {
            Self::Interval { seconds } => {
                let width = i64::from(*seconds) * 1000;
                let slot = now.timestamp_millis().div_euclid(width) * width;
                Ok(Occurrence {
                    key: slot.to_string(),
                    scheduled_at: slot,
                    local_date: now.date_naive().to_string(),
                })
            }
            Self::Daily {
                hour,
                minute,
                timezone,
            } => {
                let timezone: Tz = timezone.parse().map_err(|_| "Invalid job timezone")?;
                let mut date = now.with_timezone(&timezone).date_naive();
                let mut scheduled = daily_time(date, *hour, *minute, timezone)?;
                if scheduled > now {
                    date = date.pred_opt().ok_or("Invalid job date")?;
                    scheduled = daily_time(date, *hour, *minute, timezone)?;
                }
                Ok(Occurrence {
                    key: format!("{timezone}/{date}"),
                    scheduled_at: scheduled.timestamp_millis(),
                    local_date: now.with_timezone(&timezone).date_naive().to_string(),
                })
            }
        }
    }

    pub fn next(&self, now: i64) -> Result<i64, String> {
        let latest = self.latest(now)?;
        match self {
            Self::Interval { seconds } => latest
                .scheduled_at
                .checked_add(i64::from(*seconds) * 1000)
                .ok_or("Invalid job time".into()),
            Self::Daily {
                hour,
                minute,
                timezone,
            } => {
                let timezone: Tz = timezone.parse().map_err(|_| "Invalid job timezone")?;
                let date = DateTime::from_timestamp_millis(latest.scheduled_at)
                    .ok_or("Invalid job time")?
                    .with_timezone(&timezone)
                    .date_naive()
                    .succ_opt()
                    .ok_or("Invalid job date")?;
                Ok(daily_time(date, *hour, *minute, timezone)?.timestamp_millis())
            }
        }
    }
}

fn daily_time(
    date: NaiveDate,
    hour: u8,
    minute: u8,
    timezone: Tz,
) -> Result<DateTime<Utc>, String> {
    let time = date
        .and_hms_opt(u32::from(hour), u32::from(minute), 0)
        .ok_or("Invalid job time")?;
    // Run at the first valid minute after a spring gap. A repeated time runs once,
    // at its first occurrence. A skipped civil day is rejected explicitly.
    for offset in 0..=180 {
        let time = time
            .checked_add_signed(Duration::minutes(offset))
            .ok_or("Invalid job time")?;
        match timezone.from_local_datetime(&time) {
            LocalResult::Single(value) => return Ok(value.with_timezone(&Utc)),
            LocalResult::Ambiguous(first, second) => {
                return Ok(first.min(second).with_timezone(&Utc));
            }
            LocalResult::None => {}
        }
    }
    Err("Daily time is unavailable in this timezone".into())
}
