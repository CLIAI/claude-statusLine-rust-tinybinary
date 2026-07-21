---
title: "Producer side of statusline-parse/v1: --get-regex, field_semantics, and the single-binary decision"
date: 2026-07-21
role: producer (CLIAI/claude-statusLine-rust-tinybinary), co-designing with tuictl:mgr (consumer) and cuse:mgr (render)
status: decision + contract position — implementation not yet cut (awaiting tuictl:mgr sign-off on field_semantics)
refs:
  - uberclaude-gw/ramblings/2026-07-21--statusline-tool-as-source-of-truth-for-its-own-parse-regex.md   # Greg's seed (dev)
  - gwwtests/tuictl-mgr/tracking/2026-07-21--statusline-parse-contract-v1-proposal.md                    # tuictl:mgr consumer position
---

# Decision 1 — `--get-regex` lives in the MAIN binary, not a separate one

Greg's instinct ("separate binary to keep the main one trimmed to maximum") was worth testing.
I tested it. Conclusion: **keep it in the single binary, as a subcommand/flag.** Reasoning, from
strongest to weakest:

## 1a. Hot-path performance cost of adding it: ~zero (measured/argued, not asserted)

* The render path is: spawn → one stdin read → one `serde_json` parse → `format!` → `print!`.
  `--get-regex` is gated behind an arg-parse branch that the render path never takes. Dead code on
  the hot path **is never demand-paged in** during a normal render, so it costs no render time.
* **The expensive machinery is already compiled in.** `write_debug_log` already links
  `serde_json::to_string` + the `json!` macro (JSON *serialization*, not just parsing) and already
  does `fs::create_dir_all` + `OpenOptions` file I/O — which is exactly what `--from-setting` needs to
  read a `settings.json`. So `--get-regex`/`--from-setting` **reuse code already in the release
  binary.** The only genuinely new code is: regex-string assembly (string concat), two arg-parse
  branches, and the `field_semantics` string literals in `.rodata` — single-digit KB, none on the
  hot path.
* Baseline for Greg's "give me a number": current release binary = **444,848 bytes (434 KiB)**
  (`opt-level="s"`, thin LTO, `strip=symbols`, `panic=abort`). Expected delta from the flag:
  low single-digit KB. I'll `ls -la` diff before/after when I implement, to replace the estimate
  with a fact.

## 1b. The one real bloat risk — the `regex` crate — is avoidable

I only **emit** a regex *string*; the consumer compiles it. So the release binary needs **no** regex
engine. `regex` goes in `[dev-dependencies]`, used solely by the anti-drift self-test. **Review rule:
if `regex` ever moves to `[dependencies]`, that's the size regression to reject.**

## 1c. The operational clincher (stronger than size): discoverability + version skew

* The consumer reaches me **through `statusLine.command`**, which names *this* binary.
  `--from-setting` works precisely because it re-invokes the binary the settings file already points
  at. A second binary is **named nowhere** — the consumer would need a brand-new convention to
  discover it (PATH? sibling path?). Fresh fragility, for no gain.
* One binary **cannot** be half-upgraded into a state where the renderer and the regex-emitter
  disagree. Two binaries can. Eliminating that install-time version skew is single-binary's real
  structural win.

## 1d. Honest caveat about "cannot drift"

A single binary does **not** prevent drift *by construction* — someone can still edit the `format!`
render string and forget the regex builder. **The test is what prevents drift**, so it must be
exhaustive (see Decision 3). The gold standard is one field-spec table that BOTH render and regex-gen
consume; that's a bigger refactor deferred past v1. The cross-combo test is sufficient for v1.

# Decision 2 — the two field_semantics gotchas that contradict tuictl's draft

tuictl's proposal types `context_window = token int (1M/200k)` and implies `reset`/`cost` are simple.
My **actual** `--style full --compact` output (live, today) is:

```
Opus 4.7|max|T|c68k/200k:34%|w41%|r1321d5h|$2.31|v2.1.201
```

* **`ctx_used` / `ctx_window` are HUMAN-formatted strings, not ints.** `fmt_tokens` emits `68k`,
  `200k`, `1.2M`, `2M` (thousands as `k`, millions as `M` with 1 decimal unless whole). A consumer
  int-mapper breaks here. `field_semantics` must say: *token count, human-formatted with k/M suffix;
  un-format to get raw*.
* **`reset` is a RELATIVE duration computed at render time**, not an absolute timestamp — the live
  `r1321d5h` is `resets_at - now`. It is **not recoverable to an absolute time** from the line. Slot
  can also be empty (`--reset-status=off`, or no reset data → `||`).
* Sentinels a consumer must tolerate (all real): `model="?"`, `effort="na"`, `thinking` ∈ `T|-|?`,
  `week_pct="n/a"`, `reset=""`, `cost="$n/a"`, `version="n/a"`. The `|v<version>` slot is **optional**
  (absent under `--version-status=off`) — that optionality is the exact thing that broke the old
  hardcoded parser.

# Decision 3 — two-level versioning + test ownership

* **Two version levels, not one.** `statusline-parse/v1` = the producer-agnostic **envelope shape**
  (`{producer, contract, format_version, style, compact, regex, groups[], field_semantics}`). A
  separate **`format_version`** = the concrete line format for *this producer × these flags* (e.g.
  `claude-statusline/full-compact/1`). This is what lets Q3 generalize: a Codex/Grok/Gemini statusline
  adopts the same `statusline-parse/v1` envelope while carrying its own `format_version`. tuictl
  slightly conflated the two; splitting them is the clean fix.
* **Test ownership.** Producer (me) owns the anti-drift self-test: for every combo that changes the
  line — `style × compact × version-status{on,off} × reset-status{on,off}` — render SAMPLE and assert
  the derived regex matches with the right named captures. Consumer (tuictl) owns a contract-shape
  test (envelope keys present, `format_version` parseable, `field_semantics` covers every group).
  Shared artifact: I commit a golden `--get-regex` JSON fixture; tuictl vendors it, so a `format_version`
  bump shows up as a failing diff on their side too.

# Proposed frozen v1 groups (for `--style full --compact`)

| group        | example      | semantics                                                        |
|--------------|--------------|------------------------------------------------------------------|
| `model`      | `Opus 4.7`   | display name; may contain spaces/`.`; sentinel `?`               |
| `effort`     | `max`        | `low\|medium\|high\|xhigh\|max`; sentinel `na`                   |
| `thinking`   | `T`          | `T`=on, `-`=off, `?`=unknown                                     |
| `ctx_used`   | `68k`        | tokens, human k/M — NOT raw int                                  |
| `ctx_window` | `200k`       | tokens, human k/M — NOT raw int                                  |
| `ctx_pct`    | `34`         | int 0–100                                                        |
| `week_pct`   | `41`         | int 0–100; sentinel `n/a`                                        |
| `reset`      | `1321d5h`    | RELATIVE duration (Nd Nh / Nh Nm / now); may be empty; not absolute |
| `cost`       | `2.31`       | float; literal `$` prefix outside group; sentinel `n/a`          |
| `version`    | `2.1.201`    | x.y.z; whole `\|v…` slot optional                                |

Prefix chars (`c`, `w`, `r`, `v`, `$`) and separators are regex literals; groups capture values only.
