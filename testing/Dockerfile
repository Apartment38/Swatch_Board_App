FROM rust:1.93-trixie

RUN apt-get update && apt-get install -y --no-install-recommends \
    clang libclang-dev pkg-config qt6-base-dev qt6-qpa-plugins qt6-wayland libgl-dev weston \
    libsqlite3-dev libwayland-dev libxkbcommon-dev libdbus-1-dev \
    dbus fonts-dejavu-core python3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /work
COPY . /work
RUN --mount=type=secret,id=proxy_ca \
    if [ -f /run/secrets/proxy_ca ]; then \
      cp /run/secrets/proxy_ca /usr/local/share/ca-certificates/swatch-build-proxy.crt; \
      update-ca-certificates; \
    fi && \
    cargo fetch --locked && rustup component add clippy rustfmt && \
    rm -f /usr/local/share/ca-certificates/swatch-build-proxy.crt && \
    update-ca-certificates --fresh && \
    mkdir -p /work/target && chmod 777 /work/target && \
    chmod -R a+rX /work /usr/local/cargo && \
    chmod -R a+rwX /usr/local/cargo/registry && chmod a+w /usr/local/cargo
ENV CARGO_BUILD_JOBS=2
CMD ["bash", "testing/run-tests.sh"]
