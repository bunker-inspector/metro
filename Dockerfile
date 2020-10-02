FROM rustlang/rust:nightly
ADD . /

RUN cargo build
