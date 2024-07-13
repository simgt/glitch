FROM ubuntu:22.04 as build-gstreamer

ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    software-properties-common \
    build-essential \
    wget curl \
    bison flex \
    pkg-config \
    git \
    llvm-dev libclang-dev clang \
    ninja-build \
    libssl-dev \
    libglib2.0-dev \
    libfribidi-dev \
    libharfbuzz-dev \
    libthai-dev \
    libavfilter-dev \
    libsqlite3-dev \
    python3-pip

RUN pip3 install meson

RUN mkdir -p /work /output

ARG GSTREAMER_VERSION=1.22
RUN git clone -b ${GSTREAMER_VERSION} --depth 1 \
        https://gitlab.freedesktop.org/gstreamer/gstreamer.git \
        /work/gstreamer


WORKDIR /work/gstreamer
# Maybe use --default-library=static?
RUN CC=clang CXX=clang++ meson setup \
    --prefix=/output/gstreamer-${GSTREAMER_VERSION} \
    --buildtype=release \
    -Dgst-plugins-base:pango=disabled \
    -Dgst-devtools:tools=disabled \
    -Dgst-devtools:cairo=disabled \
    build
RUN ninja -C build install

WORKDIR /output
RUN tar -cvjSf gstreamer-${GSTREAMER_VERSION}-jammy-$(uname -m).tbz2 gstreamer-${GSTREAMER_VERSION}

FROM scratch as artifact
COPY --from=build-gstreamer /output/gstreamer-*.tbz2 /

