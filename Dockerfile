# ---- Build stage: compile the Leptos/WASM app with Trunk ----
FROM rust:1-slim-bookworm AS builder

RUN rustup target add wasm32-unknown-unknown \
    && cargo install trunk --locked

WORKDIR /app
COPY Cargo.toml Cargo.lock index.html ./
COPY src ./src

RUN trunk build --release

# ---- Runtime stage: serve the static build with nginx ----
FROM nginx:alpine AS runtime

COPY --from=builder /app/dist /usr/share/nginx/html

EXPOSE 80
