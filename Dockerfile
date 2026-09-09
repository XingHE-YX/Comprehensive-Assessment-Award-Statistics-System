FROM rust:1.88.0-bookworm AS builder
# The locked Rust driver bundles 3.46.0. Production must use the prescribed
# 3.46.1 engine; installing a newer sqlite3 CLI does not change SQLx linkage.
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config \
    && rm -rf /var/lib/apt/lists/* \
    && curl --fail --location --retry 3 https://www.sqlite.org/2024/sqlite-autoconf-3460100.tar.gz -o /tmp/sqlite.tar.gz \
    && printf '%s  %s\n' '67d3fe6d268e6eaddcae3727fce58fcc8e9c53869bdd07a0c61e38ddf2965071' '/tmp/sqlite.tar.gz' | sha256sum -c - \
    && mkdir /tmp/sqlite-source \
    && tar -xzf /tmp/sqlite.tar.gz --strip-components=1 -C /tmp/sqlite-source \
    && cd /tmp/sqlite-source \
    && CFLAGS='-O2 -fPIC -DSQLITE_CORE -DSQLITE_DEFAULT_FOREIGN_KEYS=1 -DSQLITE_ENABLE_API_ARMOR -DSQLITE_ENABLE_COLUMN_METADATA -DSQLITE_ENABLE_DBSTAT_VTAB -DSQLITE_ENABLE_FTS3 -DSQLITE_ENABLE_FTS3_PARENTHESIS -DSQLITE_ENABLE_FTS5 -DSQLITE_ENABLE_JSON1 -DSQLITE_ENABLE_LOAD_EXTENSION=1 -DSQLITE_ENABLE_MEMORY_MANAGEMENT -DSQLITE_ENABLE_RTREE -DSQLITE_ENABLE_STAT4 -DSQLITE_SOUNDEX -DSQLITE_THREADSAFE=1 -DSQLITE_USE_URI -DSQLITE_ENABLE_UNLOCK_NOTIFY -DHAVE_USLEEP=1 -DHAVE_ISNAN -D_POSIX_THREAD_SAFE_FUNCTIONS' \
       ./configure --prefix=/opt/sqlite-3.46.1 --disable-shared --enable-static --disable-readline --enable-threadsafe \
    && make -j2 && make install \
    && rm -rf /tmp/sqlite-source /tmp/sqlite.tar.gz
ENV LIBSQLITE3_SYS_USE_PKG_CONFIG=1 \
    SQLITE3_LIB_DIR=/opt/sqlite-3.46.1/lib \
    SQLITE3_INCLUDE_DIR=/opt/sqlite-3.46.1/include \
    SQLITE3_STATIC=1 \
    PKG_CONFIG_PATH=/opt/sqlite-3.46.1/lib/pkgconfig
WORKDIR /build
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
COPY migrations ./migrations
COPY templates ./templates
COPY scripts/check-sqlite-engine.sh /build/check-sqlite-engine.sh
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/build/target,sharing=locked \
    cargo build --locked --release --bin zongce-web \
    && cp /build/target/release/zongce-web /build/zongce-web \
    && ! ldd /build/zongce-web | grep -q libsqlite3
# Launch this exact release application and inspect its SQLx startup event.
# This gate cannot pass by querying the unrelated distribution sqlite3 CLI.
RUN bash /build/check-sqlite-engine.sh /build/zongce-web 3.46.1

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
