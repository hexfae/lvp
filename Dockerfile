FROM nixos/nix:2.18.3 AS builder

RUN nix-channel --update
RUN echo "experimental-features = nix-command flakes" >> /etc/nix/nix.conf

WORKDIR /app

COPY Cargo.toml Cargo.lock flake.nix flake.lock .

RUN mkdir src/

RUN echo "fn main() {}" > src/main.rs

RUN nix build

RUN rm -f target/release/deps/lvp*

COPY src src

RUN nix build

FROM busybox:glibc

COPY --from=builder /app/result/bin/lvp /app/result/bin/lvp
COPY --from=builder /app/vid /app/vid

CMD ["/app/result/bin/lvp"]

