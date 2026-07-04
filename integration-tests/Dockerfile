FROM debian:bookworm-slim

ENV DEBIAN_FRONTEND=noninteractive
ENV PATH="/root/.cargo/bin:/root/.local/bin:/root/.claude/local:/root/.claude/local/bin:${PATH}"

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        bash \
        build-essential \
        ca-certificates \
        coreutils \
        curl \
        git \
        jq \
        make \
        procps \
        python3 \
        tmux \
        xz-utils \
    && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL https://sh.rustup.rs | bash -s -- -y --profile minimal --default-toolchain stable

RUN curl -fsSL https://claude.ai/install.sh | bash

WORKDIR /work
