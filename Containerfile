# syntax=docker/dockerfile:1
FROM rustlang/rust:nightly as builder
WORKDIR /app
COPY . .
COPY .sqlx ./.sqlx
ENV SQLX_OFFLINE=true
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends libssl3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/ankan-meetup-analyser-server /app/ankan-meetup-analyser-server
COPY static ./static
COPY doc ./doc
EXPOSE 8080
ENV RUST_LOG=info
CMD ["/app/ankan-meetup-analyser-server"]
