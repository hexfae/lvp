FROM rust:latest AS cargo-build

RUN apt-get update

RUN apt-get install pkg-config libavutil-dev libavformat-dev libavdevice-dev libswscale-dev libavcodec-dev libclang-dev -y

WORKDIR /usr/src/lvp

COPY Cargo.toml Cargo.toml

RUN mkdir src/

RUN echo "fn main() {}" > src/main.rs

RUN cargo build --release

RUN rm -f target/release/deps/lvp*

COPY . .

RUN cargo build --release

# ---

FROM rust:latest

RUN apt-get update

RUN apt-get install libavutil-dev libavformat-dev libswscale-dev -y

RUN addgroup --gid 1000 lvp

RUN adduser --disabled-login --shell /bin/sh --uid 1000 --ingroup lvp lvp

WORKDIR /home/lvp/bin/

COPY --from=cargo-build /usr/src/lvp/target/release/lvp .

RUN chown lvp:lvp lvp

USER lvp

CMD ["./lvp"]

