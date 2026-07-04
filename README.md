# claude-statusline-rust-tinybinary

Tiny Rust status line renderer for Claude Code.

## Kickstarter

Claude Code status lines are most useful when they are fast and dense. This tool replaces shell pipelines like `jq | awk | date` with one tiny Rust binary that reads Claude's JSON once and prints one line.

Fast start:

```bash
make install && ./add-to-claude-settings.py --full --compact
```

That writes this kind of status line:

```text
Opus 4.7|max|T|c68k/200k:34%|w41%|r2d7h|$2.31
```

For capture while debugging missing fields such as `week` or `reset`:

```bash
make install && ./add-to-claude-settings.py --full --compact --debug-log-dir ~/.cache/claude-statusline-rust-tinybinary
```

The helper is optional. It is a portable Python 3 script using only the standard library, and the manual `settings.json` examples below show the exact configuration it writes.

## Why

Claude Code `statusLine` can run a command, passes JSON session data to that command on stdin, and displays stdout. This binary reads stdin once, parses the JSON once, and prints one compact line.

The point is lower startup and processing overhead than Bash pipelines that invoke tools such as `cat`, `jq`, `awk`, `grep`, and `date`. The implementation is intentionally simple and dependency-light.

## Styles

```bash
claude-statusline-rust-tinybinary --style default
claude-statusline-rust-tinybinary --style full
claude-statusline-rust-tinybinary --style weekly
claude-statusline-rust-tinybinary --style debug
claude-statusline-rust-tinybinary -s default
claude-statusline-rust-tinybinary --full
claude-statusline-rust-tinybinary --full --reset-status=off
claude-statusline-rust-tinybinary --full --compact
claude-statusline-rust-tinybinary --weekly -c
```

Default style is `default`.

`--compact` or `-c` is a modifier. It keeps the selected style's fields and only removes spacing, long labels, and visual separators:

```text
Opus 4.7|max|T|c34%|w41%|r2d7h
Opus 4.7|max|T|c68k/200k:34%|w41%|r2d7h|$2.31
Opus 4.7|c34%|w41%|r2d7h
```

For custom ordering, use `--format`:

```bash
claude-statusline-rust-tinybinary --format '%M|%E|%T|%w|%r|%C|%c'
```

Format tokens:

- `%M` model
- `%E` effort
- `%T` thinking
- `%w` weekly percentage
- `%r` reset status
- `%C` context summary
- `%c` cost
- `%%` literal percent sign

`--reset-status=off` hides reset output in built-in styles and makes `%r` render as empty in custom formats.

## Example output

```text
Opus 4.7 │ e:max │ T:T │ ctx ███░░░░░░░ 34% │ week 41% reset:2d7h
Opus 4.7 │ effort:max │ think:T │ ctx ███░░░░░░░ 68k/200k 34% │ week 41% reset:2d7h │ $2.31
Opus 4.7|max|T|c68k/200k:34%|w41%|r2d7h|$2.31
Opus 4.7|max|T|c68k/200k:34%|w41%||$2.31
```

Sample input:

```json
{
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
}
```

## Install

```bash
cargo install --path . --locked # or `make install`
```

Or build locally:

```bash
cargo build --release
```

## Configure Claude Code

Fast helper:

```bash
./add-to-claude-settings.py --help
./add-to-claude-settings.py --full --compact
./add-to-claude-settings.py --style default --debug-log-dir ~/.cache/claude-statusline-rust-tinybinary
```

The helper updates `~/.claude/settings.json`, preserves other top-level settings, and creates a timestamped backup when replacing an existing file.

Using `PATH`:

```json
{
  "statusLine": {
    "type": "command",
    "command": "claude-statusline-rust-tinybinary --style default",
    "padding": 0
  }
}
```

Full style with compact presentation:

```json
{
  "statusLine": {
    "type": "command",
    "command": "claude-statusline-rust-tinybinary --style full --compact",
    "padding": 0
  }
}
```

Using a direct path:

```json
{
  "statusLine": {
    "type": "command",
    "command": "~/.cargo/bin/claude-statusline-rust-tinybinary --style full",
    "padding": 0
  }
}
```

With debug capture enabled:

```json
{
  "statusLine": {
    "type": "command",
    "command": "claude-statusline-rust-tinybinary --style full --compact --debug-log-dir ~/.cache/claude-statusline-rust-tinybinary",
    "padding": 0
  }
}
```

Debug capture writes one JSONL file per status-line invocation, named like:

```text
~/.cache/claude-statusline-rust-tinybinary/260704-181530-12345.jsonl
```

Each file contains the JSON payload Claude Code passed on stdin. If the input is malformed, the file contains a small record with `bad_json` and `raw` fields. This is useful when `week` or `reset` shows `n/a`, because the captured payload shows whether Claude Code actually sent `rate_limits.seven_day.used_percentage` and `rate_limits.seven_day.resets_at`.

Fields can be null or missing, especially early in a session, after compaction, or when a model or plan does not provide a field. Missing values are handled with compact fallbacks.

## Fields used

- `model.display_name`
- `model.id`
- `effort.level`
- `thinking.enabled`
- `context_window.used_percentage`
- `context_window.total_input_tokens`
- `context_window.context_window_size`
- `rate_limits.seven_day.used_percentage`
- `rate_limits.seven_day.resets_at`
- `workspace.current_dir`
- `cwd`
- `cost.total_cost_usd`

The weekly rate limit is read from `rate_limits.seven_day.used_percentage`; it is not inferred from cost or local logs. Five-hour usage is intentionally not shown in the default layouts.

## Performance rationale

This is a small compiled CLI with one runtime dependency, `serde_json`. It avoids a shell pipeline of repeated process startup and repeated parsing. The release profile favors compact binaries for fast cold startup:

```toml
[profile.release]
opt-level = "s"
lto = "thin"
codegen-units = 1
panic = "abort"
strip = "symbols"
```

There is no TUI, daemon, background service, network call, async runtime, logging framework, or config file.

## Development

```bash
make
make build
make test
make size
make sample
make install
make uninstall
make clean
```

Manual smoke test:

```bash
echo '{"model":{"display_name":"Opus"},"context_window":{"used_percentage":34}}' \
  | target/release/claude-statusline-rust-tinybinary --style default
```

Expected shape:

```text
Opus │ e:na │ T:? │ ctx ███░░░░░░░ 34% │ week n/a
```

## References

- Claude Code status line docs: `statusLine` runs a command, receives JSON on stdin, and displays stdout.
- Cargo release profiles: `opt-level`, `lto`, `codegen-units`, `panic`, and `strip`.
