FROM rust:latest
LABEL author jianghan <jianghan@vivo.com>

# Install zsh
RUN sudo apt install -y git zsh \
    && git clone https://github.com/ohmyzsh/ohmyzsh.git ~/.oh-my-zsh \
    && cp ~/.oh-my-zsh/templates/zshrc.zsh-template ~/.zshrc \
    && sed -i "s/robbyrussell/bira/" ~/.zshrc

ENV RUSTUP_UPDATE_ROOT=https://mirrors.aliyun.com/rustup/rustup
ENV RUSTUP_DIST_SERVER=https://mirrors.aliyun.com/rustup
RUN rustup default nightly && \
    rustup target add armv7a-none-eabi && \
    rustup target add thumbv7m-none-eabi && \
    rustup target add thumbv8m.main-none-eabi && \
    rustup target add thumbv8m.main-none-eabihf && \
    cargo install cargo-binutils && \
    rustup component add llvm-tools-preview && \
    rustup component add rust-src && \
    rustup component add rustfmt
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
