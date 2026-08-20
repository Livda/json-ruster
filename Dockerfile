# ---- Build stage: compile the Leptos/WASM app with Trunk ----
# Pinned to the same version as rust-toolchain.toml, so local dev, CI and
# this image all build with an identical, reproducible Rust toolchain.
FROM rust:1.93.1-slim-bookworm AS builder

RUN rustup target add wasm32-unknown-unknown \
    && cargo install trunk --locked

WORKDIR /app
COPY Cargo.toml Cargo.lock index.html rust-toolchain.toml ./
COPY src ./src

RUN trunk build --release

# ---- Runtime stage: serve the static build with nginx ----
FROM nginx:1.27-alpine AS runtime

COPY --from=builder /app/dist /usr/share/nginx/html

EXPOSE 80
