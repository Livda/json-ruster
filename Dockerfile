# ---- Build stage: compile the Leptos/WASM app with Trunk ----
# Pinned to the same version as rust-toolchain.toml, so local dev, CI and
# this image all build with an identical, reproducible Rust toolchain.
FROM rust:1.93.1-slim-bookworm AS builder

# wasm-bindgen-cli's version must match the `wasm-bindgen` dependency in
# Cargo.toml (it provides wasm-bindgen-test-runner, used to run the
# tests/ui.rs integration suite). cargo-llvm-cov + llvm-tools-preview
# generate coverage for `cargo test --lib`. Both used by the `test`
# service in compose.yaml.
RUN rustup target add wasm32-unknown-unknown \
    && rustup component add llvm-tools-preview \
    && cargo install trunk --locked \
    && cargo install wasm-bindgen-cli --version 0.2.127 --locked \
    && cargo install cargo-llvm-cov --locked

WORKDIR /app
COPY Cargo.toml Cargo.lock index.html rust-toolchain.toml Trunk.toml ./
COPY .cargo ./.cargo
COPY src ./src
COPY tests ./tests

RUN trunk build --release

# ---- Test tooling stage: standalone chromedriver + chromium ----
# Not part of the production image -- `docker build .` with no --target
# builds the last stage (`runtime`, below), so this only gets built when
# explicitly requested (`--target chromedriver`, or the `chromedriver`
# service in compose.yaml). Used to run the wasm-bindgen-test integration
# suite (tests/ui.rs) against a real browser, locally and in CI.
FROM debian:bookworm-slim AS chromedriver

RUN apt-get update \
    && apt-get install -y --no-install-recommends chromium chromium-driver \
    && rm -rf /var/lib/apt/lists/*

EXPOSE 9515

CMD ["chromedriver", "--port=9515", "--allowed-ips=", "--allowed-origins=*"]

# ---- Runtime stage: serve the static build with nginx ----
FROM nginx:1.27-alpine AS runtime

COPY --from=builder /app/dist /usr/share/nginx/html

EXPOSE 80
