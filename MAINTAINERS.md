# Maintainers

## Compact Rendering Contract

`--compact` is a representation modifier, not a separate style.

For every built-in `--style`, compact output must keep the same ingredients as
the non-compact style and render each ingredient in its compact form. For
example, a context ingredient may change from `ctx ... 34%` to `c34%`, but it
must not disappear.

When adding, removing, or toggling a field in a built-in style:

- update the matching `render_terse` branch at the same time
- add or update unit tests for both non-compact and compact output
- keep docs examples aligned with the default visible fields

Optional display switches follow the same rule. If `--style full` includes
the Claude Code version by default, then `--style full --compact` must include
the compact Claude Code version ingredient by default too. If
`--version-status=off` hides it in full style, it must hide it in compact full
style and `%v` formats as well.

The rendered version ingredient comes from the status-line JSON `version` field.
Do not use this helper binary's Cargo package version for status output; that is
only for the standalone `--version` CLI flag.
