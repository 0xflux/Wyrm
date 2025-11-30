FROM lukemathwalker/cargo-chef:latest-rust-1.90-bookworm AS chef

FROM chef AS planner
COPY Cargo.toml ./
COPY c2 /c2
COPY client /client
COPY shared /shared
COPY shared_c2_client /shared_c2_client
COPY implant /implant
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
WORKDIR /client

COPY --from=planner /recipe.json ./recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY client/ . 
COPY ../shared /shared
COPY ../shared_c2_client /shared_c2_client

RUN cargo install trunk wasm-bindgen-cli
RUN rustup target add wasm32-unknown-unknown
RUN trunk build --release

FROM caddy:alpine AS runtime
WORKDIR /usr/share/caddy
COPY --from=builder /client/dist .
COPY client/Caddyfile /etc/caddy/Caddyfile
EXPOSE 3000
CMD ["caddy", "run", "--config", "/etc/caddy/Caddyfile"]