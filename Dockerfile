# Build stage
FROM rust:bookworm AS builder
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /app


# Copy the entire project
COPY . .

# Build the application with static linking
RUN cargo build --release --target x86_64-unknown-linux-musl

# Runtime stage
FROM scratch

# Copy the built binary from the builder stage
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/factorio-prometheus-exporter /factorio-prometheus-exporter

# Run the binary
ENTRYPOINT ["/factorio-prometheus-exporter"]