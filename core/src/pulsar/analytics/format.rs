//! Locale-independent reference formatting; unavailable always has its own representation.
pub(crate) fn number(value: f64, decimals: usize) -> String {
    if !value.is_finite() {
        return "-".into();
    }
    let decimals = decimals.min(12);
    let scale = 10_f64.powi(decimals as i32);
    // Java Formatter uses decimal half-up; unlike Rust's tie-to-even formatter.
    let rounded = value.signum() * (value.abs() * scale + 0.5).floor() / scale;
    format!("{rounded:.decimals$}")
}
pub(crate) fn value(value: Option<f64>, unit: &str) -> String {
    let Some(value) = value.filter(|v| v.is_finite()) else {
        return "-".into();
    };
    match unit {
        "%" => format!("{}%", number(value, 1)),
        "m" => format!("{} m", number(value, 2)),
        "s" => format!("{} s", number(value, 2)),
        _ => number(value, 2),
    }
}
pub(crate) fn duration(nanos: i64) -> String {
    let seconds = nanos as f64 / 1e9;
    if seconds < 60.0 {
        format!("{}s", number(seconds, 1))
    } else {
        let whole = (seconds + 0.5).floor() as i64;
        format!("{}m {:02}s", whole / 60, whole % 60)
    }
}
pub(crate) fn opponent(label: Option<&str>, id: Option<&str>) -> String {
    if let Some(label) = label.filter(|s| !s.trim().is_empty()) {
        return label.into();
    }
    if let Some(id) = id.filter(|s| !s.trim().is_empty()) {
        format!("Entity {}", id.chars().take(8).collect::<String>())
    } else {
        "Unknown".into()
    }
}
pub(crate) fn short_item(item: &str) -> String {
    item.split_once(':')
        .map(|(_, v)| v)
        .unwrap_or(item)
        .replace('_', " ")
}
pub(crate) fn result(f: &super::super::wire::FightRecord) -> String {
    let r = super::resolution(f);
    match r.classification.as_str() {
        "CONFIRMED_WIN" => "Win (confirmed)",
        "CONFIRMED_LOSS" => "Loss (confirmed)",
        "INFERRED_WIN" => "Win (inferred)",
        "INFERRED_LOSS" => "Loss (inferred)",
        "DRAW" => "Draw",
        "INTERRUPTED" => "Interrupted",
        _ => {
            if f.session.is_some() {
                "No result"
            } else {
                "Unknown"
            }
        }
    }
    .into()
}

/// Gregorian UTC date grouping; local display formatting is delegated to the host.
pub(crate) fn date_group(epoch_millis: i64) -> String {
    let days = epoch_millis.div_euclid(86_400_000) + 719468;
    let era = days.div_euclid(146097);
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[repr(C)]
struct LocalTime {
    second: i32,
    minute: i32,
    hour: i32,
    day: i32,
    month: i32,
    year: i32,
    weekday: i32,
    yearday: i32,
    daylight: i32,
    #[cfg(unix)]
    offset: std::ffi::c_long,
    #[cfg(unix)]
    zone: *const std::ffi::c_char,
}
fn local_time(epoch_millis: i64) -> Option<LocalTime> {
    let seconds = epoch_millis.div_euclid(1000);
    // The C runtimes write a fully initialized tm only on success. No shared
    // static localtime buffer is used by these concurrently invoked projections.
    let mut result = std::mem::MaybeUninit::<LocalTime>::uninit();
    #[cfg(windows)]
    {
        #[link(name = "ucrt")]
        unsafe extern "C" {
            fn _localtime64_s(out: *mut LocalTime, time: *const i64) -> i32;
        }
        if unsafe { _localtime64_s(result.as_mut_ptr(), &seconds) } == 0 {
            return Some(unsafe { result.assume_init() });
        }
    }
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn localtime_r(time: *const std::ffi::c_long, out: *mut LocalTime) -> *mut LocalTime;
        }
        let seconds = std::ffi::c_long::try_from(seconds).ok()?;
        if !unsafe { localtime_r(&seconds, result.as_mut_ptr()) }.is_null() {
            return Some(unsafe { result.assume_init() });
        }
    }
    None
}
#[allow(dead_code)]
pub(crate) fn date_time(epoch_millis: i64) -> String {
    local_time(epoch_millis)
        .map(|t| {
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}",
                i64::from(t.year) + 1900,
                t.month + 1,
                t.day,
                t.hour,
                t.minute
            )
        })
        .unwrap_or_else(|| "-".into())
}
pub(crate) fn short_date(epoch_millis: i64) -> String {
    local_time(epoch_millis)
        .map(|t| {
            format!(
                "{:02}-{:02} {:02}:{:02}",
                t.month + 1,
                t.day,
                t.hour,
                t.minute
            )
        })
        .unwrap_or_else(|| "-".into())
}
