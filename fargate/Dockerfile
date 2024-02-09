FROM rust:alpine3.18  as build

WORKDIR /dice-app

COPY . .

RUN  apk update \
     && apk add --no-cache musl-dev \
     && cargo build --target x86_64-unknown-linux-musl --release

FROM alpine:3.18.3

RUN apk update \
    && apk add openssl ca-certificates

COPY --from=build /dice-app/target/x86_64-unknown-linux-musl/release/dice-app /usr/local/bin

ENTRYPOINT [ "/usr/local/bin/dice-app" ] 