FROM rust:buster
RUN apt-get update &&\
    apt-get install -y curl protobuf-compiler
RUN cargo install cargo-chef
WORKDIR build
