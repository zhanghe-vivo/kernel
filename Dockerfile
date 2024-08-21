FROM rust:latest
LABEL author jianghan <jianghan@vivo.com>

ENV RUSTUP_DIST_SERVER https://mirrors.tuna.tsinghua.edu.cn/rustup
ENV RUSTUP_UPDATE_ROOT https://mirrors.tuna.tsinghua.edu.cn/rustup/rustup
RUN rustup default nightly && \
    rustup target add armv7a-none-eabi && \
    cargo install cargo-binutils && \
    rustup component add llvm-tools-preview && \
    rustup component add rust-src \
    rustup component add rustfmt \
ADD sources.list /etc/apt/
RUN DEBIAN_FRONTEND=noninteractive apt-get update -y && \
    apt-get install git   wget bzip2 \
    build-essential  libncurses-dev  cppcheck   \
    gcc-arm-none-eabi gdb-arm-none-eabi binutils-arm-none-eabi  qemu-system-arm    \
    python3-pip  python3-requests  -y   \
    scons \
    libclang-dev && \
    apt-get clean -y

CMD ["bash"]
