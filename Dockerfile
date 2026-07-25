FROM rust:1.97-slim-bookworm

ARG USER_ID=1000
ARG GROUP_ID=1000

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        python3 \
        python3-dev \
        python3-venv \
    && rm -rf /var/lib/apt/lists/* \
    && rustup component add clippy rustfmt \
    && cargo install maturin --version 1.14.1 --locked \
    && groupadd --gid "${GROUP_ID}" developer \
    && useradd \
        --uid "${USER_ID}" \
        --gid "${GROUP_ID}" \
        --create-home \
        developer \
    && mkdir -p /home/developer/.cargo /home/developer/target /workspace \
    && chown -R developer:developer /home/developer /workspace

USER developer

ENV CARGO_HOME=/home/developer/.cargo
ENV CARGO_TARGET_DIR=/home/developer/target

WORKDIR /workspace

CMD ["cargo", "test", "--workspace"]
