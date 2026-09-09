FROM rust:1.88.0-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
COPY migrations ./migrations
COPY templates ./templates
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/build/target,sharing=locked \
    cargo build --locked --release --bin zongce-web \
    && cp /build/target/release/zongce-web /build/zongce-web

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl sqlite3 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 zongce \
    && useradd --uid 10001 --gid 10001 --no-create-home zongce \
    && mkdir -p /app /data /uploads /backups \
    && chown 10001:10001 /data /uploads /backups
WORKDIR /app
COPY --from=builder /build/zongce-web /usr/local/bin/zongce-web
COPY static ./static
USER 10001:10001
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD curl --fail --silent --output /dev/null http://127.0.0.1:3000/healthz || exit 1
ENTRYPOINT ["/usr/local/bin/zongce-web"]
