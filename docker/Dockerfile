# syntax=docker/dockerfile:1
# Multi-stage build: the builder compiles a static-ish release binary,
# the runtime copies it into a minimal image. Final image ~20 MB.
#
# Build:  docker build -t seedcanvas/seed-canvas .
# Run:    docker run --rm -p 8080:8080 -v "$PWD:/data" seedcanvas/seed-canvas \
#             serve --addr 0.0.0.0:8080 --root /data

FROM rust:1.83-slim AS builder
WORKDIR /build

# Cache dependencies: copy manifests first, build a stub workspace.
COPY Cargo.toml Cargo.lock ./
COPY crates/seed-canvas-core/Cargo.toml crates/seed-canvas-core/
COPY crates/seed-canvas-adapter-server/Cargo.toml crates/seed-canvas-adapter-server/
COPY crates/seed-canvas-adapter-svg/Cargo.toml crates/seed-canvas-adapter-svg/
COPY crates/seed-canvas-storage/Cargo.toml crates/seed-canvas-storage/
COPY crates/seed-canvas-server/Cargo.toml crates/seed-canvas-server/
COPY crates/seed-canvas-embed/Cargo.toml crates/seed-canvas-embed/
COPY crates/seed-canvas-registry/Cargo.toml crates/seed-canvas-registry/
COPY crates/seed-canvas-cli/Cargo.toml crates/seed-canvas-cli/
COPY examples/galaxy/Cargo.toml examples/galaxy/
COPY examples/particles/Cargo.toml examples/particles/
COPY examples/mandala/Cargo.toml examples/mandala/
RUN mkdir -p crates/seed-canvas-core/src crates/seed-canvas-adapter-server/src \
        crates/seed-canvas-adapter-svg/src crates/seed-canvas-storage/src \
        crates/seed-canvas-server/src crates/seed-canvas-embed/src \
        crates/seed-canvas-registry/src crates/seed-canvas-cli/src \
        examples/galaxy/src examples/particles/src examples/mandala/src \
    && echo "pub fn placeholder() {}" > crates/seed-canvas-core/src/lib.rs \
    && for c in seed-canvas-adapter-server seed-canvas-adapter-svg seed-canvas-storage \
                seed-canvas-server seed-canvas-embed seed-canvas-registry; do \
           echo "pub fn placeholder() {}" > "crates/$c/src/lib.rs"; \
       done \
    && echo "fn main() {}" > crates/seed-canvas-cli/src/main.rs \
    && echo "pub fn placeholder() {}" > examples/galaxy/src/lib.rs \
    && echo "fn main() {}" > examples/galaxy/src/main.rs \
    && echo "pub fn placeholder() {}" > examples/particles/src/lib.rs \
    && echo "fn main() {}" > examples/particles/src/main.rs \
    && echo "pub fn placeholder() {}" > examples/mandala/src/lib.rs \
    && echo "fn main() {}" > examples/mandala/src/main.rs \
    && cargo build --release --workspace \
    || true

# Now copy the real sources (registry index is include_str!'d by core-adjacent crates).
COPY rust-toolchain.toml ./
COPY crates crates
COPY examples examples
COPY registry registry
COPY README.md ./
# Touch every source file so cargo rebuilds with the real code.
RUN find crates examples -name "*.rs" -exec touch {} \; \
    && cargo build --release --workspace -p seed-canvas-cli

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/seed-canvas /usr/local/bin/seed-canvas

# Non-root runtime user; the gallery workspace is mounted at /data.
RUN useradd --system --create-home --shell /usr/sbin/nologin seedcanvas \
    && mkdir -p /data && chown seedcanvas:seedcanvas /data
USER seedcanvas
WORKDIR /data

EXPOSE 8080
VOLUME ["/data"]

# Serve the gallery by default; override with any other subcommand.
ENTRYPOINT ["seed-canvas"]
CMD ["serve", "--addr", "0.0.0.0:8080", "--root", "/data"]