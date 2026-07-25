# syntax=docker/dockerfile:1

FROM rust:1.97-slim-bookworm

RUN \
    --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && \
    apt-get install --yes --no-install-recommends \
        libusb-1.0-0 \
        python3 \
        python3-dev \
        python3-venv && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

RUN rustup component add clippy rustfmt

RUN \
    --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/tmp/maturin-target \
    CARGO_TARGET_DIR=/tmp/maturin-target \
    cargo install maturin --version 1.14.1 --locked

ARG USER_ID=1000
ARG GROUP_ID=1000

RUN groupadd --gid "${GROUP_ID}" developer \
    && useradd \
        --uid "${USER_ID}" \
        --gid "${GROUP_ID}" \
        --create-home \
        developer \
    && mkdir -p \
        /home/developer/.cargo \
        /home/developer/target \
        /workspace/.venv \
    && chown -R developer:developer /home/developer /workspace

USER developer

ENV CARGO_HOME=/home/developer/.cargo
ENV CARGO_TARGET_DIR=/home/developer/target

WORKDIR /workspace

CMD ["cargo", "test", "--workspace"]
