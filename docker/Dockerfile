FROM rust:1.83.0
LABEL author jianghan <jianghan@vivo.com>

ENV RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static \
    RUSTUP_UPDATE_ROOT=https://mirrors.ustc.edu.cn/rust-static/rustup \
    LC_ALL=en_US.UTF-8 \
    LANG=en_US.UTF-8

ADD sources.list /etc/apt/
RUN DEBIAN_FRONTEND=noninteractive apt-get update -y && \
    apt-get install git   wget bzip2 \
    build-essential  libncurses-dev  cppcheck \
    gcc-arm-none-eabi gdb-arm-none-eabi binutils-arm-none-eabi  qemu-system-arm \
    python3-pip  python3-requests \
    scons libclang-dev zsh -y \
    && apt-get clean -y

RUN rustup toolchain install nightly-2024-11-22 && \
    rustup default nightly-2024-11-22 && \
    rustup target add armv7a-none-eabi && \
    rustup target add thumbv7m-none-eabi && \
    rustup target add thumbv7em-none-eabihf && \
    rustup target add thumbv8m.main-none-eabi && \
    rustup target add thumbv8m.main-none-eabihf && \
    cargo install cargo-binutils && \
    rustup component add llvm-tools-preview && \
    rustup component add rust-src && \
    rustup component add rustfmt && \
    rustup component add clippy

# Install ohmyzsh
# RUN git clone https://github.com/ohmyzsh/ohmyzsh.git ~/.oh-my-zsh \
#     && cp ~/.oh-my-zsh/templates/zshrc.zsh-template ~/.zshrc \
#     && sed -i "s/robbyrussell/agnoster/" ~/.zshrc \
#     && git clone https://github.com/zsh-users/zsh-autosuggestions ${ZSH_CUSTOM:-~/.oh-my-zsh/custom}/plugins/zsh-autosuggestions \
#     && git clone https://github.com/zsh-users/zsh-syntax-highlighting.git ${ZSH_CUSTOM:-~/.oh-my-zsh/custom}/plugins/zsh-syntax-highlighting \
#     && sed -i "s/plugins=(git.*)$/plugins=(git zsh-syntax-highlighting zsh-autosuggestions)/" ~/.zshrc

CMD ["/bin/zsh"]