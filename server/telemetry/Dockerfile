# Multi-stage build: compile in builder, copy to slim runtime image.
FROM rust:1.79-alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev pkgconfig

WORKDIR /build
COPY . .
RUN cargo build --release --bin vox-server

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM alpine:3.20

RUN apk add --no-cache libgcc ca-certificates

COPY --from=builder /build/target/release/vox-server /usr/local/bin/vox-server
COPY --from=builder /build/contracts/ /app/contracts/

EXPOSE 4318

ENV LISTEN_ADDR=0.0.0.0:4318
ENV RUST_LOG=info

ENTRYPOINT ["vox-server"]
