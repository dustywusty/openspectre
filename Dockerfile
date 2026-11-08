FROM rust:1.90-bookworm AS wasm
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN rustup target add wasm32-unknown-unknown \
    && cargo test --locked --workspace \
    && cargo build --locked -p spectre-core --release --target wasm32-unknown-unknown

FROM node:22-bookworm-slim AS runtime
ENV NODE_ENV=production NATIVE_HOST=0.0.0.0 NATIVE_PORT=8080 SPECTRE_ASSETS=open
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci --omit=dev --ignore-scripts && npm cache clean --force
COPY native-server.mjs multiplayer-server.mjs asset-pack.mjs ./
COPY native-web ./native-web
COPY assets/open/generated ./assets/open/generated
COPY --from=wasm /build/target/wasm32-unknown-unknown/release/spectre_core.wasm ./native-web/spectre_core.wasm
USER node
EXPOSE 8080
HEALTHCHECK --interval=15s --timeout=3s --start-period=10s --retries=3 CMD node -e "fetch('http://127.0.0.1:8080/pack.json').then(r=>{if(!r.ok)process.exit(1)}).catch(()=>process.exit(1))"
CMD ["node", "native-server.mjs"]
