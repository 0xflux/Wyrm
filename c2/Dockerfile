FROM lukemathwalker/cargo-chef:latest-rust-1.90-bookworm AS chef
WORKDIR /app

FROM chef AS planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder 
WORKDIR /app

COPY --from=planner /app/recipe.json ./recipe.json
RUN cargo +nightly chef cook --release -p c2 --recipe-path recipe.json
COPY . .
RUN cargo +nightly build -p c2 --release

FROM rust:1.90-bookworm AS runtime

RUN echo "Installing environment dependencies..."
RUN apt update -qq
RUN apt install -qq -y build-essential \
    pkg-config libssl-dev gcc-mingw-w64-x86-64 \
    g++-mingw-w64-x86-64 curl libgtk-3-dev clang p7zip-full \ 
    clang lld llvm

RUN set -eux; \
    if command -v llvm-lib-14   >/dev/null 2>&1; then ln -sf /usr/bin/llvm-lib-14   /usr/bin/llvm-lib; fi; \
    if command -v ld.lld        >/dev/null 2>&1; then ln -sf /usr/bin/ld.lld        /usr/bin/lld-link; fi; \
    if command -v clang         >/dev/null 2>&1; then ln -sf /usr/bin/clang         /usr/bin/clang-cl; fi

RUN echo "Installing toolchains..."
RUN rustup toolchain install nightly && rustup component add llvm-tools

RUN rustup target add x86_64-pc-windows-msvc
RUN rustup target add x86_64-pc-windows-msvc --toolchain nightly
RUN cargo install cargo-xwin

RUN rustup override set nightly

EXPOSE 8087
WORKDIR /app
VOLUME ["/data"]

COPY --from=builder /app/target/release/c2 .
COPY --from=builder /app/c2/profiles/ ./profiles
COPY --from=builder /app/implant/ ./implant/
COPY --from=builder /app/shared_no_std/ ./shared_no_std/
COPY --from=builder /app/shared/ ./shared/
COPY --from=builder /app/loader/ ./loader/

RUN mkdir -p /app/implant/.tmp
ENV TMPDIR=/app/implant/.tmp

ENTRYPOINT ["/app/c2"]