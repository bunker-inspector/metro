FROM rustlang/rust:nightly-alpine3.12
WORKDIR ~
ADD . .

RUN apk add musl-dev

RUN cargo build
