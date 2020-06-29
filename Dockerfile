FROM ekidd/rust-musl-builder:1.35.0-openssl11 as builder

COPY Cargo.toml Cargo.lock ua_regexes.yaml ./
COPY src ./src

# build project
RUN cargo build --release --features bundled
RUN strip target/x86_64-unknown-linux-musl/release/szaklon-api

FROM alpine:3.9.4
RUN apk --no-cache add ca-certificates
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/szaklon-api /
COPY config-prod.toml /config.toml
# Uncomment to copy certs
# COPY key.pem cert.pem /

EXPOSE 9876
EXPOSE 9877

ENV RUST_LOG=info
CMD ["/szaklon-api"]
