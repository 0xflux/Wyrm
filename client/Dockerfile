FROM rust:1.90-alpine AS builder
RUN rustup target add x86_64-unknown-linux-musl
RUN apk add gcc musl-dev libffi-dev openssl-dev

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo "fn main(){}" > src/main.rs
RUN cargo build --release --target x86_64-unknown-linux-musl

COPY src ./src
COPY static ./static
COPY templates ./templates

RUN cargo build --release --target=x86_64-unknown-linux-musl 

FROM alpine:latest
EXPOSE 4040

WORKDIR /app

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/wyrm_gui .
COPY --from=builder /app/static ./static
COPY --from=builder /app/templates ./templates

CMD ["./wyrm_gui"]