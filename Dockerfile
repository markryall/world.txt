# world.txt in a small container, for hosting on Fly.io (see "Hosting on
# Fly.io" in the README). The world itself lives on a volume at /data.

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
    && apt-get install -y --no-install-recommends ca-certificates libssl3 sqlite3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/world-txt /usr/local/bin/world.txt
COPY deploy/fly-start.sh /usr/local/bin/fly-start.sh
ENV WORLD_DB=/data/world.db
WORKDIR /data
EXPOSE 8080
CMD ["/usr/local/bin/fly-start.sh"]
