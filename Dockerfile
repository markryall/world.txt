# world.txt in a small container, for an always-on machine: a home server with
# Docker (docs/hosting-at-home.md) or a rented one like Fly.io
# (docs/hosting-fly.md). The world itself lives on a volume at /data.

FROM rust:1.99-bookworm AS build
WORKDIR /src
# Dependencies first, so a code change doesn't rebuild them all.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && cargo build --release && rm -rf src
COPY src src
COPY web web
COPY stations.txt stations.txt
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 sqlite3 tzdata \
    && rm -rf /var/lib/apt/lists/*
# The world runs as this ordinary user, not root (start.sh hands over to
# it), so a bug in the server can only reach the world's own files.
RUN useradd --system --uid 10001 --home-dir /data --no-create-home --shell /usr/sbin/nologin world
COPY --from=build /src/target/release/world-txt /usr/local/bin/world.txt
COPY deploy/start.sh /usr/local/bin/start.sh
ENV WORLD_DB=/data/world.db
WORKDIR /data
EXPOSE 8080
CMD ["/usr/local/bin/start.sh"]
