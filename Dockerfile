FROM rust:1.99-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends perl make gcc && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY locales ./locales
COPY migrations ./migrations
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates ffmpeg && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 bot
WORKDIR /app
COPY --from=builder /build/target/release/whatsapp-bot /app/whatsapp-bot
COPY config.example.toml /app/config.example.toml
RUN mkdir /app/data && chown -R bot:bot /app
USER bot
EXPOSE 3000
ENTRYPOINT ["/app/whatsapp-bot"]
CMD ["--config", "/app/config.toml", "run"]

