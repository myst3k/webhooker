FROM rust:bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src/ src/
COPY migrations/ migrations/
COPY templates/ templates/
COPY static/ static/

ENV SQLX_OFFLINE=true
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/webhooker /usr/local/bin/webhooker
COPY --from=builder /app/static /app/static
COPY --from=builder /app/migrations /app/migrations

WORKDIR /app

EXPOSE 3000
CMD ["webhooker"]
