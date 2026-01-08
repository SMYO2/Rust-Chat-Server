# 1️⃣ Gebruik de officiële Rust image (met tooling)
FROM rust:latest AS builder


# 2️⃣ Installeer dependencies die sommige crates nodig hebben
RUN apt-get update && apt-get install -y pkg-config libssl-dev

# 3️⃣ Stel de werkdirectory in
WORKDIR /app

# 4️⃣ Kopieer Cargo-bestanden en download dependencies eerst (voor betere caching)
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release || true

# 5️⃣ Kopieer nu de rest van de code
COPY . .

# 6️⃣ Bouw het project in release mode
RUN cargo build --release

# 7️⃣ Gebruik een lichtere runtime-image
FROM debian:bookworm-slim

# 8️⃣ Stel werkdirectory in voor runtime
WORKDIR /app

# 9️⃣ Kopieer de gecompileerde binary uit de builder
COPY --from=builder /app/target/release/Rustserver /app/

# 🔟 Start het programma
CMD ["./Rustserver"]
