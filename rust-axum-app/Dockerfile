FROM --platform=${BUILDPLATFORM} docker.io/library/rust:1.96.0@sha256:fb328f0f58becb23ba1719940a2c94ece8b0b48afa837d05b79ef64bc1e18f6e AS builder
ARG TARGETARCH
ARG TARGETPLATFORM
ARG BUILDPLATFORM


RUN echo Building on ${BUILDPLATFORM} for ${TARGETPLATFORM}

RUN apt-get update && \
  if [ "${TARGETPLATFORM}" = "linux/arm64" ] && [ "${BUILDPLATFORM}" != "linux/arm64" ]; then \
  apt-get install --no-install-recommends -y g++-aarch64-linux-gnu libc6-dev-arm64-cross libprotobuf-dev protobuf-compiler ca-certificates && \
  rustup target add aarch64-unknown-linux-gnu; \
  else \
  apt-get install --no-install-recommends -y g++ libc6-dev libprotobuf-dev protobuf-compiler ca-certificates; \
  fi


WORKDIR /app/

COPY ./rust-axum-app/ /app/

# Use offline mode so sqlx reads from the .sqlx/ cache instead of connecting to a live DB
ENV SQLX_OFFLINE=true

# Build - cross-compile for ARM64 or native build
RUN if [ "${TARGETPLATFORM}" = "linux/arm64" ] && [ "${BUILDPLATFORM}" != "linux/arm64" ] ; then \
  env CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
  CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
  CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++ \
  cargo build -r --target aarch64-unknown-linux-gnu && \
  cp /app/target/aarch64-unknown-linux-gnu/release/api /app/target/release/api; \
  else \
  cargo build -r; \
  fi

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:e1fd250ce83d94603e9887ec991156a6c26905a6b0001039b7a43699018c0733

WORKDIR /app

COPY --from=builder /app/target/release/api api

EXPOSE ${RUST_APP_PORT}

CMD ["./api"]
