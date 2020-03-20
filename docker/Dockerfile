FROM ekidd/rust-musl-builder

WORKDIR /home/rust

RUN curl -L -o elm.gz https://github.com/elm/compiler/releases/download/0.19.1/binary-for-linux-64-bit.gz
RUN gunzip elm.gz
RUN chmod +x elm
RUN sudo mv elm /usr/local/bin

RUN rustup target add i686-unknown-linux-musl
