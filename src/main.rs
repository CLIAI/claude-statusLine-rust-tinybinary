use serde_json::Value;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Read};
use std::path::Path;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Compact,
    Full,
    Weekly,
    Debug,
}

#[derive(Debug, PartialEq)]
struct Options {
    style: Style,
    format: Option<String>,
    show_reset: bool,
    debug_log_dir: Option<String>,
    terse: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            style: Style::Compact,
            format: None,
            show_reset: true,
            debug_log_dir: None,
            terse: false,
        }
    }
}

#[derive(Debug, PartialEq)]
struct Status {
    model: String,
    effort: String,
    thinking: String,
    ctx_pct: u64,
    ctx_tokens: u64,
    ctx_window: u64,
    week_pct: Option<f64>,
    week_reset: Option<u64>,
    cost_usd: Option<f64>,
    cwd: Option<String>,
}

impl Status {
    fn from_json(v: &Value) -> Self {
        let model = str_path(v, &["model", "display_name"])
            .or_else(|| str_path(v, &["model", "id"]))
            .unwrap_or("?")
            .to_string();

        let effort = str_path(v, &["effort", "level"])
            .unwrap_or("na")
            .to_string();

        let thinking = match bool_path(v, &["thinking", "enabled"]) {
            Some(true) => "T",
            Some(false) => "-",
            None => "?",
        }
        .to_string();

        let ctx_pct = pct_u64(num_path(v, &["context_window", "used_percentage"]).unwrap_or(0.0));
        let ctx_tokens = u64_path(v, &["context_window", "total_input_tokens"]).unwrap_or(0);
        let ctx_window = u64_path(v, &["context_window", "context_window_size"]).unwrap_or(200_000);

        let week_pct = num_path(v, &["rate_limits", "seven_day", "used_percentage"]);
        let week_reset = u64_path(v, &["rate_limits", "seven_day", "resets_at"]);
        let cost_usd = num_path(v, &["cost", "total_cost_usd"]);
        let cwd = str_path(v, &["workspace", "current_dir"])
            .or_else(|| str_path(v, &["cwd"]))
            .map(str::to_string)
            .filter(|s| !s.is_empty());

        Self {
            model,
            effort,
            thinking,
            ctx_pct,
            ctx_tokens,
            ctx_window,
            week_pct,
            week_reset,
            cost_usd,
            cwd,
        }
    }
}

fn main() {
    let options = match parse_options(env::args()) {
        Ok(options) => options,
        Err(msg) => {
            eprintln!("{msg}");
            process::exit(2);
        }
    };

    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        print!("cc-status: stdin-error");
        return;
    }

    let value: Value = match serde_json::from_str(&input) {
        Ok(value) => {
            write_debug_log(options.debug_log_dir.as_deref(), &input, Some(&value));
            value
        }
        Err(_) => {
            write_debug_log(options.debug_log_dir.as_deref(), &input, None);
            print!("cc-status: bad-json");
            return;
        }
    };

    let status = Status::from_json(&value);
    print!("{}", render(&options, &status));
}

fn parse_options<I, S>(args: I) -> Result<Options, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut iter = args.into_iter();
    let _program = iter.next();
    let mut options = Options::default();

    while let Some(arg) = iter.next() {
        match arg.as_ref() {
            "--style" | "-s" => {
                let Some(value) = iter.next() else {
                    return Err(usage("missing style"));
                };
                options.style = parse_style_name(value.as_ref())?;
            }
            value if value.starts_with("--style=") => {
                options.style = parse_style_name(&value["--style=".len()..])?;
            }
            "--compact" | "-c" => options.terse = true,
            "--full" => options.style = Style::Full,
            "--weekly" => options.style = Style::Weekly,
            "--debug" => options.style = Style::Debug,
            "--format" => {
                let Some(value) = iter.next() else {
                    return Err(usage("missing format"));
                };
                options.format = Some(value.as_ref().to_string());
            }
            value if value.starts_with("--format=") => {
                options.format = Some(value["--format=".len()..].to_string());
            }
            "--debug-log-dir" => {
                let Some(value) = iter.next() else {
                    return Err(usage("missing debug log dir"));
                };
                options.debug_log_dir = Some(value.as_ref().to_string());
            }
            value if value.starts_with("--debug-log-dir=") => {
                options.debug_log_dir = Some(value["--debug-log-dir=".len()..].to_string());
            }
            "--reset-status=on" => options.show_reset = true,
            "--reset-status=off" => options.show_reset = false,
            "--reset-status" => {
                let Some(value) = iter.next() else {
                    return Err(usage("missing reset status"));
                };
                options.show_reset = parse_on_off(value.as_ref())?;
            }
            value if value.starts_with("--reset-status=") => {
                options.show_reset = parse_on_off(&value["--reset-status=".len()..])?;
            }
            "--help" | "-h" => return Err(usage("usage")),
            other => return Err(usage(&format!("unknown argument: {other}"))),
        }
    }

    Ok(options)
}

fn parse_style_name(name: &str) -> Result<Style, String> {
    match name {
        "compact" => Ok(Style::Compact),
        "full" => Ok(Style::Full),
        "weekly" => Ok(Style::Weekly),
        "debug" => Ok(Style::Debug),
        other => Err(usage(&format!("unknown style: {other}"))),
    }
}

fn parse_on_off(value: &str) -> Result<bool, String> {
    match value {
        "on" => Ok(true),
        "off" => Ok(false),
        other => Err(usage(&format!("unknown reset status: {other}"))),
    }
}

fn usage(prefix: &str) -> String {
    format!(
        "{prefix}\nusage: claude-statusline-rust-tinybinary [--style compact|full|weekly|debug] [--compact|-c] [--reset-status on|off] [--format FORMAT] [--debug-log-dir DIR]"
    )
}

fn render(options: &Options, s: &Status) -> String {
    render_at(options, s, None)
}

fn render_at(options: &Options, s: &Status, now: Option<u64>) -> String {
    if let Some(format) = &options.format {
        return render_format(format, s, now, options.show_reset);
    }

    if options.terse {
        return render_terse(options.style, s, now, options.show_reset);
    }

    match options.style {
        Style::Compact => format!(
            "{} │ e:{} │ T:{} │ ctx {} {}% │ week {}",
            s.model,
            s.effort,
            s.thinking,
            bar(s.ctx_pct, 10),
            s.ctx_pct,
            fmt_week_compact(s.week_pct, s.week_reset, now, options.show_reset)
        ),
        Style::Full => format!(
            "{} │ effort:{} │ think:{} │ ctx {} {}/{} {}% │ week {} │ {}",
            s.model,
            s.effort,
            s.thinking,
            bar(s.ctx_pct, 10),
            fmt_tokens(s.ctx_tokens),
            fmt_tokens(s.ctx_window),
            s.ctx_pct,
            fmt_week_full(s.week_pct, s.week_reset, now, options.show_reset),
            fmt_cost(s.cost_usd)
        ),
        Style::Weekly => format!(
            "{} │ ctx {}% │ week {}",
            s.model,
            s.ctx_pct,
            fmt_week_full(s.week_pct, s.week_reset, now, options.show_reset)
        ),
        Style::Debug => format!(
            "model={} effort={} thinking={} ctx_pct={} ctx_tokens={} ctx_window={} week_pct={} week_reset={}",
            s.model,
            s.effort,
            s.thinking,
            s.ctx_pct,
            s.ctx_tokens,
            s.ctx_window,
            s.week_pct
                .map(fmt_percent)
                .unwrap_or_else(|| "n/a".to_string()),
            s.week_reset
                .map(|reset| reset.to_string())
                .unwrap_or_else(|| "n/a".to_string())
        ),
    }
}

fn render_terse(style: Style, s: &Status, now: Option<u64>, show_reset: bool) -> String {
    let reset = if show_reset {
        fmt_reset_at(s.week_reset, now)
            .map(|reset| format!("r{reset}"))
            .unwrap_or_default()
    } else {
        String::new()
    };

    match style {
        Style::Compact => format!(
            "{}|{}|{}|c{}%|w{}|{}",
            s.model,
            s.effort,
            s.thinking,
            s.ctx_pct,
            fmt_week_pct(s.week_pct),
            reset
        ),
        Style::Full => format!(
            "{}|{}|{}|c{}/{}:{}%|w{}|{}|{}",
            s.model,
            s.effort,
            s.thinking,
            fmt_tokens(s.ctx_tokens),
            fmt_tokens(s.ctx_window),
            s.ctx_pct,
            fmt_week_pct(s.week_pct),
            reset,
            fmt_cost(s.cost_usd)
        ),
        Style::Weekly => format!(
            "{}|c{}%|w{}|{}",
            s.model,
            s.ctx_pct,
            fmt_week_pct(s.week_pct),
            reset
        ),
        Style::Debug => format!(
            "m={}|e={}|t={}|cp={}|ct={}|cw={}|wp={}|wr={}",
            s.model,
            s.effort,
            s.thinking,
            s.ctx_pct,
            s.ctx_tokens,
            s.ctx_window,
            s.week_pct
                .map(fmt_percent)
                .unwrap_or_else(|| "n/a".to_string()),
            s.week_reset
                .map(|reset| reset.to_string())
                .unwrap_or_else(|| "n/a".to_string())
        ),
    }
}

fn render_format(format: &str, s: &Status, now: Option<u64>, show_reset: bool) -> String {
    let mut out = String::new();
    let mut chars = format.chars();

    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }

        match chars.next() {
            Some('%') => out.push('%'),
            Some('M') => out.push_str(&s.model),
            Some('E') => out.push_str(&s.effort),
            Some('T') => out.push_str(&s.thinking),
            Some('w') => out.push_str(&fmt_week_pct(s.week_pct)),
            Some('r') => {
                if show_reset {
                    out.push_str(&fmt_reset_label(s.week_reset, now));
                }
            }
            Some('C') => out.push_str(&format!("ctx {} {}%", bar(s.ctx_pct, 10), s.ctx_pct)),
            Some('c') => out.push_str(&fmt_cost(s.cost_usd)),
            Some(other) => {
                out.push('%');
                out.push(other);
            }
            None => out.push('%'),
        }
    }

    out
}

fn fmt_week_compact(
    week_pct: Option<f64>,
    week_reset: Option<u64>,
    now: Option<u64>,
    show_reset: bool,
) -> String {
    match week_pct {
        Some(pct) if show_reset => match fmt_reset_at(week_reset, now) {
            Some(reset) => format!("{}% reset:{reset}", fmt_percent(pct)),
            None => format!("{}%", fmt_percent(pct)),
        },
        Some(pct) => format!("{}%", fmt_percent(pct)),
        None => "n/a".to_string(),
    }
}

fn fmt_week_full(
    week_pct: Option<f64>,
    week_reset: Option<u64>,
    now: Option<u64>,
    show_reset: bool,
) -> String {
    match week_pct {
        Some(pct) if show_reset => {
            let reset = fmt_reset_at(week_reset, now).unwrap_or_else(|| "n/a".to_string());
            format!("{}% reset:{reset}", fmt_percent(pct))
        }
        Some(pct) => format!("{}%", fmt_percent(pct)),
        None if show_reset => "n/a reset:n/a".to_string(),
        None => "n/a".to_string(),
    }
}

fn fmt_week_pct(week_pct: Option<f64>) -> String {
    week_pct
        .map(|pct| format!("{}%", fmt_percent(pct)))
        .unwrap_or_else(|| "n/a".to_string())
}

fn fmt_reset_label(reset_at: Option<u64>, now: Option<u64>) -> String {
    fmt_reset_at(reset_at, now)
        .map(|reset| format!("reset:{reset}"))
        .unwrap_or_else(|| "reset:n/a".to_string())
}

fn fmt_cost(cost: Option<f64>) -> String {
    match cost {
        Some(cost) if cost.is_finite() && cost >= 0.0 => format!("${cost:.2}"),
        _ => "$n/a".to_string(),
    }
}

fn write_debug_log(dir: Option<&str>, input: &str, parsed: Option<&Value>) {
    let Some(dir) = dir else {
        return;
    };
    let dir = Path::new(dir);
    if fs::create_dir_all(dir).is_err() {
        return;
    }

    let Some(now) = current_epoch_seconds() else {
        return;
    };
    let name = format!("{}-{}.jsonl", fmt_timestamp(now), process::id());
    let path = dir.join(name);
    let line = match parsed {
        Some(value) => serde_json::to_string(value).ok(),
        None => serde_json::to_string(&serde_json::json!({
            "bad_json": true,
            "raw": input
        }))
        .ok(),
    };

    let Some(line) = line else {
        return;
    };

    if let Ok(mut file) = OpenOptions::new().create_new(true).write(true).open(path) {
        use std::io::Write;
        let _ = writeln!(file, "{line}");
    }
}

fn fmt_timestamp(epoch_seconds: u64) -> String {
    let days = epoch_seconds / 86_400;
    let seconds = epoch_seconds % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    let hour = seconds / 3_600;
    let minute = (seconds % 3_600) / 60;
    let second = seconds % 60;
    format!(
        "{:02}{:02}{:02}-{:02}{:02}{:02}",
        year.rem_euclid(100),
        month,
        day,
        hour,
        minute,
        second
    )
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}

fn str_path<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut cur = v;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str()
}

fn bool_path(v: &Value, path: &[&str]) -> Option<bool> {
    let mut cur = v;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_bool()
}

fn num_path(v: &Value, path: &[&str]) -> Option<f64> {
    let mut cur = v;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_f64()
}

fn u64_path(v: &Value, path: &[&str]) -> Option<u64> {
    let mut cur = v;
    for key in path {
        cur = cur.get(*key)?;
    }

    cur.as_u64()
        .or_else(|| {
            cur.as_i64()
                .and_then(|n| if n >= 0 { Some(n as u64) } else { None })
        })
        .or_else(|| {
            cur.as_f64().and_then(|n| {
                if n.is_finite() && n >= 0.0 {
                    Some(n as u64)
                } else {
                    None
                }
            })
        })
}

fn pct_u64(value: f64) -> u64 {
    if value.is_finite() {
        value.round().clamp(0.0, 100.0) as u64
    } else {
        0
    }
}

fn fmt_percent(value: f64) -> String {
    pct_u64(value).to_string()
}

fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        let value = n as f64 / 1_000_000.0;
        if n % 1_000_000 == 0 {
            format!("{value:.0}M")
        } else {
            format!("{value:.1}M")
        }
    } else if n >= 1_000 {
        format!("{}k", n / 1_000)
    } else {
        n.to_string()
    }
}

fn bar(percent: u64, width: usize) -> String {
    let percent = percent.min(100);
    let filled = (percent as usize * width + 50) / 100;
    let empty = width.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

fn fmt_reset(reset_at: Option<u64>) -> Option<String> {
    fmt_reset_at(reset_at, current_epoch_seconds())
}

fn fmt_reset_at(reset_at: Option<u64>, now: Option<u64>) -> Option<String> {
    let reset_at = reset_at?;
    let Some(now) = now else {
        return fmt_reset(Some(reset_at));
    };
    Some(fmt_duration(reset_at.saturating_sub(now)))
}

fn current_epoch_seconds() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

fn fmt_duration(seconds: u64) -> String {
    if seconds == 0 {
        return "now".to_string();
    }

    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let mins = (seconds % 3_600) / 60;

    if days > 0 {
        format!("{days}d{hours}h")
    } else if hours > 0 {
        format!("{hours}h{mins}m")
    } else if mins > 0 {
        format!("{mins}m")
    } else {
        "now".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SAMPLE_JSON: &str = r#"{
      "model": {
        "id": "claude-opus-4-7",
        "display_name": "Opus 4.7"
      },
      "effort": {
        "level": "max"
      },
      "thinking": {
        "enabled": true
      },
      "context_window": {
        "used_percentage": 34.2,
        "total_input_tokens": 68000,
        "context_window_size": 200000
      },
      "rate_limits": {
        "seven_day": {
          "used_percentage": 41.4,
          "resets_at": 1898780400
        }
      },
      "workspace": {
        "current_dir": "/home/greg/project"
      },
      "cost": {
        "total_cost_usd": 2.31
      }
    }"#;

    const SAMPLE_NOW: u64 = 1_898_780_400 - (2 * 86_400 + 7 * 3_600);

    fn options(style: Style) -> Options {
        Options {
            style,
            ..Options::default()
        }
    }

    fn sample_status() -> Status {
        let value: Value = serde_json::from_str(SAMPLE_JSON).unwrap();
        Status::from_json(&value)
    }

    #[test]
    fn formats_tokens() {
        assert_eq!(fmt_tokens(68), "68");
        assert_eq!(fmt_tokens(68_000), "68k");
        assert_eq!(fmt_tokens(200_000), "200k");
        assert_eq!(fmt_tokens(1_200_000), "1.2M");
        assert_eq!(fmt_tokens(2_000_000), "2M");
    }

    #[test]
    fn clamps_percent() {
        assert_eq!(pct_u64(-10.0), 0);
        assert_eq!(pct_u64(34.2), 34);
        assert_eq!(pct_u64(34.6), 35);
        assert_eq!(pct_u64(101.0), 100);
        assert_eq!(pct_u64(f64::NAN), 0);
    }

    #[test]
    fn renders_bar() {
        assert_eq!(bar(0, 10), "░░░░░░░░░░");
        assert_eq!(bar(34, 10), "███░░░░░░░");
        assert_eq!(bar(100, 10), "██████████");
        assert_eq!(bar(250, 10), "██████████");
    }

    #[test]
    fn formats_duration() {
        assert_eq!(fmt_duration(2 * 86_400 + 7 * 3_600), "2d7h");
        assert_eq!(fmt_duration(5 * 3_600 + 12 * 60), "5h12m");
        assert_eq!(fmt_duration(43 * 60), "43m");
        assert_eq!(fmt_duration(0), "now");
        assert_eq!(fmt_duration(31), "now");
    }

    #[test]
    fn renders_sample_compact_output() {
        let status = sample_status();

        assert_eq!(
            render_at(&options(Style::Compact), &status, Some(SAMPLE_NOW)),
            "Opus 4.7 │ e:max │ T:T │ ctx ███░░░░░░░ 34% │ week 41% reset:2d7h"
        );
    }

    #[test]
    fn renders_sample_full_output() {
        let status = sample_status();

        assert_eq!(
            render_at(&options(Style::Full), &status, Some(SAMPLE_NOW)),
            "Opus 4.7 │ effort:max │ think:T │ ctx ███░░░░░░░ 68k/200k 34% │ week 41% reset:2d7h │ $2.31"
        );
    }

    #[test]
    fn renders_sample_weekly_output() {
        let status = sample_status();

        assert_eq!(
            render_at(&options(Style::Weekly), &status, Some(SAMPLE_NOW)),
            "Opus 4.7 │ ctx 34% │ week 41% reset:2d7h"
        );
    }

    #[test]
    fn renders_sample_debug_output() {
        let status = sample_status();

        assert_eq!(
            render_at(&options(Style::Debug), &status, Some(SAMPLE_NOW)),
            "model=Opus 4.7 effort=max thinking=T ctx_pct=34 ctx_tokens=68000 ctx_window=200000 week_pct=41 week_reset=1898780400"
        );
    }

    #[test]
    fn renders_terse_compact_output() {
        let status = sample_status();
        let render_options = Options {
            terse: true,
            ..Options::default()
        };

        assert_eq!(
            render_at(&render_options, &status, Some(SAMPLE_NOW)),
            "Opus 4.7|max|T|c34%|w41%|r2d7h"
        );
    }

    #[test]
    fn renders_terse_full_output_with_full_fields() {
        let status = sample_status();
        let render_options = Options {
            style: Style::Full,
            terse: true,
            ..Options::default()
        };

        assert_eq!(
            render_at(&render_options, &status, Some(SAMPLE_NOW)),
            "Opus 4.7|max|T|c68k/200k:34%|w41%|r2d7h|$2.31"
        );
    }

    #[test]
    fn renders_terse_weekly_output_with_weekly_fields() {
        let status = sample_status();
        let render_options = Options {
            style: Style::Weekly,
            terse: true,
            ..Options::default()
        };

        assert_eq!(
            render_at(&render_options, &status, Some(SAMPLE_NOW)),
            "Opus 4.7|c34%|w41%|r2d7h"
        );
    }

    #[test]
    fn renders_terse_debug_output_with_debug_fields() {
        let status = sample_status();
        let render_options = Options {
            style: Style::Debug,
            terse: true,
            ..Options::default()
        };

        assert_eq!(
            render_at(&render_options, &status, Some(SAMPLE_NOW)),
            "m=Opus 4.7|e=max|t=T|cp=34|ct=68000|cw=200000|wp=41|wr=1898780400"
        );
    }

    #[test]
    fn renders_terse_output_with_empty_reset_slot() {
        let value: Value = serde_json::from_str(
            r#"{"effort":{"level":"xhigh"},"thinking":{"enabled":true},"context_window":{"used_percentage":55},"rate_limits":{"seven_day":{"used_percentage":12}}}"#,
        )
        .unwrap();
        let status = Status::from_json(&value);
        let render_options = Options {
            terse: true,
            ..Options::default()
        };

        assert_eq!(
            render_at(&render_options, &status, Some(SAMPLE_NOW)),
            "?|xhigh|T|c55%|w12%|"
        );
    }

    #[test]
    fn terse_output_respects_hidden_reset_status() {
        let status = sample_status();
        let render_options = Options {
            terse: true,
            show_reset: false,
            ..Options::default()
        };

        assert_eq!(
            render_at(&render_options, &status, Some(SAMPLE_NOW)),
            "Opus 4.7|max|T|c34%|w41%|"
        );
    }

    #[test]
    fn renders_minimal_compact_output_with_fallbacks() {
        let value: Value = serde_json::from_str(
            r#"{"model":{"display_name":"Opus"},"context_window":{"used_percentage":34}}"#,
        )
        .unwrap();
        let status = Status::from_json(&value);

        assert_eq!(
            render_at(&options(Style::Compact), &status, Some(SAMPLE_NOW)),
            "Opus │ e:na │ T:? │ ctx ███░░░░░░░ 34% │ week n/a"
        );
    }

    #[test]
    fn can_hide_reset_status() {
        let status = sample_status();
        let render_options = Options {
            style: Style::Full,
            show_reset: false,
            ..Options::default()
        };

        assert_eq!(
            render_at(&render_options, &status, Some(SAMPLE_NOW)),
            "Opus 4.7 │ effort:max │ think:T │ ctx ███░░░░░░░ 68k/200k 34% │ week 41% │ $2.31"
        );
    }

    #[test]
    fn renders_custom_format_output() {
        let status = sample_status();
        let render_options = Options {
            format: Some("%M|%E|%T|%w|%r|%C|%c".to_string()),
            ..Options::default()
        };

        assert_eq!(
            render_at(&render_options, &status, Some(SAMPLE_NOW)),
            "Opus 4.7|max|T|41%|reset:2d7h|ctx ███░░░░░░░ 34%|$2.31"
        );
    }

    #[test]
    fn custom_format_respects_hidden_reset_status() {
        let status = sample_status();
        let render_options = Options {
            format: Some("%M|%w|%r|%C".to_string()),
            show_reset: false,
            ..Options::default()
        };

        assert_eq!(
            render_at(&render_options, &status, Some(SAMPLE_NOW)),
            "Opus 4.7|41%||ctx ███░░░░░░░ 34%"
        );
    }

    #[test]
    fn extracts_json_with_fallbacks() {
        let value = json!({
            "model": { "id": "claude-opus-4-7" },
            "context_window": { "used_percentage": 34.2 },
            "cwd": "/tmp/project"
        });
        let status = Status::from_json(&value);

        assert_eq!(status.model, "claude-opus-4-7");
        assert_eq!(status.effort, "na");
        assert_eq!(status.thinking, "?");
        assert_eq!(status.ctx_pct, 34);
        assert_eq!(status.ctx_tokens, 0);
        assert_eq!(status.ctx_window, 200_000);
        assert_eq!(status.week_pct, None);
        assert_eq!(status.cwd.as_deref(), Some("/tmp/project"));
    }

    #[test]
    fn parses_styles() {
        assert_eq!(
            parse_options(["claude-statusline-rust-tinybinary", "--style", "full"])
                .unwrap()
                .style,
            Style::Full
        );
        assert_eq!(
            parse_options(["claude-statusline-rust-tinybinary", "-s", "weekly"])
                .unwrap()
                .style,
            Style::Weekly
        );
        assert_eq!(
            parse_options(["claude-statusline-rust-tinybinary"])
                .unwrap()
                .style,
            Style::Compact
        );
        assert!(parse_options(["claude-statusline-rust-tinybinary", "--style", "nope"]).is_err());
    }

    #[test]
    fn parses_terse_compact_flags() {
        assert!(
            parse_options(["claude-statusline-rust-tinybinary", "--compact"])
                .unwrap()
                .terse
        );
        assert!(
            parse_options(["claude-statusline-rust-tinybinary", "-c"])
                .unwrap()
                .terse
        );
    }

    #[test]
    fn parses_option_flags() {
        let parsed = parse_options([
            "claude-statusline-rust-tinybinary",
            "--full",
            "--reset-status=off",
            "--format",
            "%M|%w",
            "--debug-log-dir=/tmp/status-json",
        ])
        .unwrap();

        assert_eq!(parsed.style, Style::Full);
        assert_eq!(parsed.format.as_deref(), Some("%M|%w"));
        assert!(!parsed.show_reset);
        assert_eq!(parsed.debug_log_dir.as_deref(), Some("/tmp/status-json"));
    }

    #[test]
    fn formats_debug_log_timestamp() {
        assert_eq!(fmt_timestamp(0), "700101-000000");
        assert_eq!(fmt_timestamp(1_704_067_200), "240101-000000");
    }

    #[test]
    fn writes_debug_log_jsonl() {
        let dir = std::env::temp_dir().join(format!(
            "claude-statusline-rust-tinybinary-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        let value: Value = serde_json::from_str(SAMPLE_JSON).unwrap();

        write_debug_log(dir.to_str(), SAMPLE_JSON, Some(&value));

        let mut entries = fs::read_dir(&dir).unwrap();
        let path = entries.next().unwrap().unwrap().path();
        assert!(entries.next().is_none());
        assert!(path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with(".jsonl"));

        let logged = fs::read_to_string(&path).unwrap();
        let logged_value: Value = serde_json::from_str(logged.trim_end()).unwrap();
        assert_eq!(logged_value["model"]["display_name"], "Opus 4.7");

        let _ = fs::remove_dir_all(&dir);
    }
}
