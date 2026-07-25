FROM rust:1.97-slim-bookworm

ARG USER_ID=1000
ARG GROUP_ID=1000

RUN rustup component add clippy rustfmt \
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
