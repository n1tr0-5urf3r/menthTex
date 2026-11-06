FROM node:26-trixie-slim AS frontend-build

WORKDIR /app/frontend

ARG VITE_BASE_PATH=/
ENV VITE_BASE_PATH=${VITE_BASE_PATH}

COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci

COPY frontend/ ./
RUN npm run build


FROM rust:1.99-trixie AS backend-build

WORKDIR /app/backend

RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake \
    && rm -rf /var/lib/apt/lists/*

COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src
COPY backend/migrations ./migrations
RUN cargo build --release


FROM debian:trixie-slim

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates poppler-utils \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /app/storage

COPY --from=backend-build /app/backend/target/release/backend /app/backend
COPY --from=frontend-build /app/frontend/dist /app/frontend-dist
COPY conferences.bib /app/conferences.bib

ENV PORT=3000
ENV FRONTEND_DIST=/app/frontend-dist
ENV CONFERENCES_BIB_PATH=/app/conferences.bib
ENV STORAGE_ROOT=/app/storage

EXPOSE 3000

CMD ["/app/backend"]
