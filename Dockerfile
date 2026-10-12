# syntax=docker/dockerfile:1
# check=error=true

# The image for open.wenmarpro.com: the open-server binary, the data file and
# the shop file that was matched against it.
#
# Build: docker build --build-arg DATA_VERSION=2026.09 --build-arg SHOP_VERSION=2026.10.12 -t wenmar-open .
# The data file must be at data/build/wenmar-open-<DATA_VERSION>.sqlite3
# (mise run data) and the shop file at
# data/build/shop/wenmar-open-shop-<SHOP_VERSION>.sqlite3 (bin/shop-pull).
# Deploy: see docs/deploy.md.

# The image tag pins the Rust version and already contains that toolchain, so
# rustup never has to reach static.rust-lang.org. Keep it in sync with the
# channel in rust-toolchain.toml and the rust pin in mise.toml.
FROM rust:1.99.0-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
# The rates of the Canadian invoice page, which the server's build reads.
COPY data/rates ./data/rates
# The reference tables of the tools, which the server's build reads.
COPY data/reference ./data/reference
# The makers' own links, which the server's build reads and checks.
COPY data/catalog ./data/catalog
# The rules for repair shops, which the server's build reads.
COPY data/rules ./data/rules
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked -p open-server \
    && cp target/release/open-server /usr/local/bin/open-server

FROM debian:bookworm-slim AS runtime
ARG DATA_VERSION
ARG SHOP_VERSION
RUN groupadd --system --gid 1000 open \
    && useradd open --uid 1000 --gid 1000 --no-create-home --shell /usr/sbin/nologin
# Create the data directory first. COPY --chmod would otherwise apply 0444 to
# the parent directories it creates implicitly, stripping their execute bit so
# the unprivileged user could not reach the file.
RUN install -d -m 0755 /app/data
COPY --from=builder /usr/local/bin/open-server /usr/local/bin/open-server
# Read-only for everyone: the server never writes to it.
COPY --chmod=0444 data/build/wenmar-open-${DATA_VERSION}.sqlite3 /app/data/wenmar-open.sqlite3
# The shop file: recalls and the other datasets beside the catalog. It holds
# the catalog ids of the data file above and of no other.
COPY --chmod=0444 data/build/shop/wenmar-open-shop-${SHOP_VERSION}.sqlite3 /app/data/wenmar-open-shop.sqlite3

USER 1000:1000
ENV OPEN_DATA=/app/data/wenmar-open.sqlite3 \
    OPEN_SHOP_DATA=/app/data/wenmar-open-shop.sqlite3 \
    PORT=3000 \
    OPEN_TRUSTED_PROXIES=1 \
    RUST_LOG=info

# Opens both files exactly as the server will. A missing file, one of another
# schema version, one that is not a data file, or a shop file that was matched
# against another data file fails the build.
RUN ["/usr/local/bin/open-server", "check"]

EXPOSE 3000
CMD ["/usr/local/bin/open-server"]
