# Build the frontend
FROM node:22-bookworm-slim AS web
WORKDIR /app/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# Build the server
FROM rust:1-bookworm AS server
WORKDIR /app/server
COPY server/ ./
RUN cargo build --release --locked

# Runtime image: the server binary plus the built frontend
FROM debian:bookworm-slim
WORKDIR /app
COPY --from=server /app/server/target/release/tcgbars-server /usr/local/bin/tcgbars-server
COPY --from=web /app/web/dist ./web/dist

# The SQLite file lives in /data; mount a persistent volume there.
RUN mkdir -p /data
ENV STATIC_DIR=/app/web/dist \
    DATABASE_URL=sqlite:///data/tcgbars.db \
    PORT=8080
EXPOSE 8080
CMD ["tcgbars-server"]
