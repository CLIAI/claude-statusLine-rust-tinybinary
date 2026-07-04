use serde_json::Value;
use std::env;
use std::io::{self, Read};
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
    let style = match parse_style(env::args()) {
        Ok(style) => style,
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
        Ok(value) => value,
        Err(_) => {
            print!("cc-status: bad-json");
            return;
        }
    };

    let status = Status::from_json(&value);
    print!("{}", render(style, &status));
}

fn parse_style<I, S>(args: I) -> Result<Style, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut iter = args.into_iter();
    let _program = iter.next();
    let mut style = Style::Compact;

    while let Some(arg) = iter.next() {
        match arg.as_ref() {
            "--style" | "-s" => {
                let Some(value) = iter.next() else {
                    return Err(usage("missing style"));
                };
                style = parse_style_name(value.as_ref())?;
            }
            "--help" | "-h" => return Err(usage("usage")),
            other => return Err(usage(&format!("unknown argument: {other}"))),
        }
    }

    Ok(style)
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

fn usage(prefix: &str) -> String {
    format!("{prefix}\nusage: claude-statusline [--style compact|full|weekly|debug]")
}

fn render(style: Style, s: &Status) -> String {
    match style {
        Style::Compact => format!(
            "{} │ e:{} │ T:{} │ ctx {} {}% │ week {}",
            s.model,
            s.effort,
            s.thinking,
            bar(s.ctx_pct, 10),
            s.ctx_pct,
            fmt_week_compact(s.week_pct, s.week_reset)
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
            fmt_week_full(s.week_pct, s.week_reset),
            fmt_cost(s.cost_usd)
        ),
        Style::Weekly => format!(
            "{} │ ctx {}% │ week {}",
            s.model,
            s.ctx_pct,
            fmt_week_full(s.week_pct, s.week_reset)
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

fn fmt_week_compact(week_pct: Option<f64>, week_reset: Option<u64>) -> String {
    match week_pct {
        Some(pct) => match fmt_reset(week_reset) {
            Some(reset) => format!("{}% reset:{reset}", fmt_percent(pct)),
            None => format!("{}%", fmt_percent(pct)),
        },
        None => "n/a".to_string(),
    }
}

fn fmt_week_full(week_pct: Option<f64>, week_reset: Option<u64>) -> String {
    match week_pct {
        Some(pct) => {
            let reset = fmt_reset(week_reset).unwrap_or_else(|| "n/a".to_string());
            format!("{}% reset:{reset}", fmt_percent(pct))
        }
        None => "n/a reset:n/a".to_string(),
    }
}

fn fmt_cost(cost: Option<f64>) -> String {
    match cost {
        Some(cost) if cost.is_finite() && cost >= 0.0 => format!("${cost:.2}"),
        _ => "$n/a".to_string(),
    }
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
    let reset_at = reset_at?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(fmt_duration(reset_at.saturating_sub(now)))
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
            parse_style(["claude-statusline", "--style", "full"]).unwrap(),
            Style::Full
        );
        assert_eq!(
            parse_style(["claude-statusline", "-s", "weekly"]).unwrap(),
            Style::Weekly
        );
        assert_eq!(parse_style(["claude-statusline"]).unwrap(), Style::Compact);
        assert!(parse_style(["claude-statusline", "--style", "nope"]).is_err());
    }
}
