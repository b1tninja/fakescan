# Multi-stage build: control plane + WSD Scan Service stub.
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY profiles ./profiles
RUN cargo build --release -p fakescan \
    && strip target/release/fakescan

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 fakescan
WORKDIR /app
COPY --from=build /src/target/release/fakescan /app/fakescan
COPY profiles /app/profiles
USER fakescan
ENV RUST_LOG=fakescan=info
EXPOSE 3920/tcp
# WS-Discovery multicast (optional; often needs --network host on Linux)
EXPOSE 3702/udp
HEALTHCHECK --interval=15s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -fsS http://127.0.0.1:3920/health || exit 1
ENTRYPOINT ["/app/fakescan"]
# Discovery binds UDP 3702; under bridge networking Hello may not reach the LAN.
# Use --network host (Linux) and --advertise <LAN-IP> for Windows WSD install tests.
CMD ["--listen", "0.0.0.0:3920", "--profiles", "/app/profiles", "--profile", "brother-wsd"]
