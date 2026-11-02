# syntax=docker/dockerfile:1
FROM rust:1.95-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends protobuf-compiler libprotobuf-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY web ./web
COPY distribution ./distribution
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/build/target \
    touch src/main.rs && cargo build --locked --release && cp target/release/rightcommit /rightcommit

FROM node:22-bookworm-slim
ARG CODEX_VERSION=0.165.0
ARG CLAUDE_VERSION=2.1.312
ARG PI_VERSION=1.4.0
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates git curl bash python3 ripgrep jq util-linux tini build-essential pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/* \
    && npm install --global @openai/codex@${CODEX_VERSION} @anthropic-ai/claude-code@${CLAUDE_VERSION} @earendil-works/pi-coding-agent@${PI_VERSION} \
    && npm cache clean --force
COPY pi-extensions/package.json pi-extensions/package-lock.json pi-extensions/.npmrc /app/pi-extensions/
RUN cd /app/pi-extensions && npm ci --omit=dev --no-audit --no-fund && npm cache clean --force
COPY pi-extensions/web-fetch.ts /app/pi-extensions/
COPY scripts/install-twg.sh /tmp/install-twg.sh
RUN sh /tmp/install-twg.sh && rm /tmp/install-twg.sh
COPY --from=build /usr/local/rustup /opt/rustup
COPY --from=build /usr/local/cargo/bin/rustup /usr/local/bin/rustup
RUN for bin in cargo rustc rustdoc rustfmt cargo-fmt; do ln -s /usr/local/bin/rustup /usr/local/bin/$bin; done \
    && chmod -R a+rX /opt/rustup
COPY --from=build /rightcommit /usr/local/bin/rightcommit
# MCP example servers saved before the rename still run /usr/local/bin/nodo-studio.
RUN ln -s rightcommit /usr/local/bin/nodo-studio
COPY examples /app/examples
WORKDIR /app
ENV NODO_DATA=/data NODO_ASSETS=/app NODO_LISTEN=0.0.0.0:8080 NODO_ISOLATE_USERS=1 RUSTUP_HOME=/opt/rustup RUST_LOG=rightcommit=info
VOLUME /data
EXPOSE 8080
HEALTHCHECK --interval=20s --timeout=3s CMD curl --fail --silent http://127.0.0.1:8080/healthz || exit 1
ENTRYPOINT ["/usr/bin/tini", "-g", "--", "/usr/local/bin/rightcommit"]
