# Integration Tests

These tests build an isolated Docker image, install the project tooling, install Claude Code with:

```bash
curl -fsSL https://claude.ai/install.sh | bash
```

Then the runner copies the current repository into the running container, compiles it, runs `claude update`, and starts Claude Code inside fixed-size `tmux` panes. Each scenario writes a different `statusLine` configuration to `/root/.claude/settings.json` and checks `tmux capture-pane` output plus status-line debug JSONL files.

## Commands

```bash
make integration-tests
make integration-tests-keep-docker-artifacts
make clean-docker-artifacts
```

`make integration-tests` removes the container after the run. It copies captures and debug logs to `integration-tests/tmp`.

`make integration-tests-keep-docker-artifacts` leaves the container and image in place for inspection. Use `make clean-docker-artifacts` afterward.

## Auth

The container is isolated by default. The tmux phase requires Claude Code to reach the interactive prompt, so real Claude auth must be available inside the container. If Claude Code stops at an auth screen, the status line is not invoked and the test fails with captures under `integration-tests/tmp`.

The runner handles the first-run theme prompt and the API-key confirmation prompt, but it does not choose a login provider for you.

## Current WIP

`make integration-tests` currently fails on a machine that does not provide Claude auth to the Docker container. That failure is intentional: the test must not pass unless Claude Code reaches the interactive prompt and actually invokes the configured `statusLine` command.

TODO: implement `make claude-integration-test-auth` as a separate, explicit helper for preparing the isolated container auth path. That target should make it clear whether it uses `ANTHROPIC_API_KEY`, a mounted Claude config/token, or another supported Claude Code auth mechanism. Keep this separate from `make integration-tests` so the default test command does not silently copy host credentials into Docker.

Pass any required environment or mounts through Docker like this:

```bash
INTEGRATION_TESTS_DOCKER_ARGS="-e ANTHROPIC_API_KEY" make integration-tests
```

Keep this opt-in so the default run does not copy host Claude configuration into the container.

## What Is Tested

- The Docker image installs Python 3, tmux, supporting CLI tools, and current stable Rust/Cargo before installing Claude Code.
- The repository is copied into the already-provisioned container, then built with `cargo build --release`.
- Rust unit tests run with `cargo test`.
- `claude update` runs after the local build.
- Claude Code is launched in tmux at multiple pane sizes.
- Different `statusLine` commands are written and exercised:
  - `--style default`
  - `--style full --compact`
  - `--style weekly`
  - custom `--format`

## Artifacts

Artifacts are written under `integration-tests/tmp`:

- `claude-version.txt`
- `claude-update.txt`
- `capture-*.txt`
- `debug-*/*.jsonl`
- `failure-*.txt`

The capture files are intentionally plain text so failures can be inspected with normal shell tools.
