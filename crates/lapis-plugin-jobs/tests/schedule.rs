use chrono::DateTime;
use lapis_plugin_jobs::Schedule;

fn time(value: &str) -> i64 {
    DateTime::parse_from_rfc3339(value)
        .unwrap()
        .timestamp_millis()
}
fn daily(hour: u8, minute: u8) -> Schedule {
    Schedule::Daily {
        hour,
        minute,
        timezone: "Europe/London".into(),
    }
}

#[test]
fn validates_schedule_boundaries() {
    assert!(Schedule::Interval { seconds: 59 }.validate().is_err());
    assert!(Schedule::Interval { seconds: 60 }.validate().is_ok());
    assert!(
        Schedule::Interval { seconds: 2_678_401 }
            .validate()
            .is_err()
    );
    assert!(daily(24, 0).validate().is_err());
    assert!(daily(9, 60).validate().is_err());
    assert!(
        Schedule::Daily {
            hour: 9,
            minute: 0,
            timezone: "unknown".into()
        }
        .validate()
        .is_err()
    );
}

#[test]
fn intervals_collapse_missed_slots_and_use_wall_clock_boundaries() {
    let schedule = Schedule::Interval { seconds: 300 };
    let now = time("2026-11-24T12:02:15Z");
    assert_eq!(
        schedule.latest(now).unwrap().scheduled_at,
        time("2026-11-24T12:00:00Z")
    );
    assert_eq!(schedule.next(now).unwrap(), time("2026-11-24T12:05:00Z"));
    assert_eq!(
        schedule.latest(now + 60000).unwrap(),
        schedule.latest(now).unwrap()
    );
    assert_eq!(
        schedule.latest(now + 86400000).unwrap().scheduled_at,
        time("2026-11-25T12:00:00Z")
    );
}

#[test]
fn daily_catchup_and_next_run_follow_configured_timezone() {
    let schedule = daily(9, 0);
    assert_eq!(
        schedule
            .latest(time("2026-07-15T07:59:00Z"))
            .unwrap()
            .scheduled_at,
        time("2026-07-14T08:00:00Z")
    );
    assert_eq!(
        schedule.next(time("2026-07-15T07:59:00Z")).unwrap(),
        time("2026-07-15T08:00:00Z")
    );
    let occurrence = schedule.latest(time("2026-07-15T23:30:00Z")).unwrap();
    assert_eq!(occurrence.local_date, "2026-07-16");
    assert_eq!(occurrence.key, "Europe/London/2026-07-15");
}

#[test]
fn repeated_autumn_time_has_one_occurrence() {
    let schedule = daily(1, 30);
    let first = schedule.latest(time("2026-10-25T00:45:00Z")).unwrap();
    let second = schedule.latest(time("2026-10-25T01:45:00Z")).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.scheduled_at, time("2026-10-25T00:30:00Z"));
    assert_eq!(
        schedule.next(time("2026-10-25T00:45:00Z")).unwrap(),
        time("2026-10-26T01:30:00Z")
    );
}

#[test]
fn missing_spring_time_runs_at_first_available_minute() {
    let schedule = daily(1, 30);
    assert_eq!(
        schedule.next(time("2026-03-29T00:45:00Z")).unwrap(),
        time("2026-03-29T01:00:00Z")
    );
    assert_eq!(
        schedule
            .latest(time("2026-03-29T01:05:00Z"))
            .unwrap()
            .scheduled_at,
        time("2026-03-29T01:00:00Z")
    );
}
