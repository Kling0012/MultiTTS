FROM rust:1.75-bullseye as builder
WORKDIR /usr/src/multitts
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release && rm -r src
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y ffmpeg ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/multitts/target/release/t /usr/local/bin/multitts
WORKDIR /usr/src/multitts
CMD ["/usr/local/bin/multitts"]
