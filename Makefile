.PHONY: all build test install uninstall clean size sample

BIN := target/release/claude-statusline
SAMPLE_JSON := {"model":{"id":"claude-opus-4-7","display_name":"Opus 4.7"},"effort":{"level":"max"},"thinking":{"enabled":true},"context_window":{"used_percentage":34.2,"total_input_tokens":68000,"context_window_size":200000},"rate_limits":{"seven_day":{"used_percentage":41.4,"resets_at":1898780400}},"workspace":{"current_dir":"/home/greg/project"},"cost":{"total_cost_usd":2.31}}

all: build

build:
	cargo build --release

test:
	cargo test

install:
	cargo install --path . --locked

uninstall:
	cargo uninstall claude-statusline

clean:
	cargo clean

size: build
	ls -lh $(BIN)

sample: build
	@printf '%s\n' '$(SAMPLE_JSON)' | $(BIN) --style compact
	@printf '\n'
	@printf '%s\n' '$(SAMPLE_JSON)' | $(BIN) --style full
	@printf '\n'
	@printf '%s\n' '$(SAMPLE_JSON)' | $(BIN) --style weekly
	@printf '\n'
	@printf '%s\n' '$(SAMPLE_JSON)' | $(BIN) --style debug
	@printf '\n'
