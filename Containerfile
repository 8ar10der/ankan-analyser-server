# syntax=docker/dockerfile:1
FROM rust:1.77 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
WORKDIR /app
COPY --from=builder /app/target/release/ankan-meetup-analyser-server /app/ankan-meetup-analyser-server
COPY static ./static
COPY doc ./doc
EXPOSE 8080
ENV RUST_LOG=info
CMD ["/app/ankan-meetup-analyser-server"]

