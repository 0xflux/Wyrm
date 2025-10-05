FROM lukemathwalker/cargo-chef:latest-rust-1.90 AS chef
WORKDIR /app

FROM chef AS planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder 
WORKDIR /app
COPY --from=planner /app/recipe.json ./recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim AS runtime
EXPOSE 4040

WORKDIR /app

COPY --from=builder /app/target/release/wyrm_gui .
COPY --from=builder /app/static ./static
COPY --from=builder /app/templates ./templates

ENTRYPOINT ["/app/wyrm_gui"]