FROM amd64/rust:1.63

WORKDIR /opt

RUN curl -L -o elm.gz https://github.com/elm/compiler/releases/download/0.19.1/binary-for-linux-64-bit.gz
RUN gunzip elm.gz
RUN chmod +x elm
RUN mv elm /usr/local/bin

RUN rustup target add x86_64-unknown-linux-musl

RUN apt-get update
RUN apt-get install -y musl-tools
