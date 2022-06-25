FROM rust:buster as shuttle-common
RUN apt-get update &&\
    apt-get install -y curl
