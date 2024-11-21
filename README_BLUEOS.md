# BlueOS
BlueOS是vivo自研的实时操作系统。

## 准备工作

### 安装gcc交叉编译工具链
官方指南 https://learn.arm.com/install-guides/gcc/cross/
#### 类Debian发行版
```
sudo apt update
sudo apt install gcc-arm-none-eabi -y
sudo apt install gcc-arm-linux-gnueabihf -y
sudo apt install gcc-aarch64-linux-gnu -y
```
#### 类Fedora发行版
```
sudo dnf update -y
sudo dnf install arm-none-eabi-gcc-cs -y
sudo dnf install arm-none-eabi-newlib -y
sudo dnf install gcc-aarch64-linux-gnu -y
sudo dnf install gcc-arm-linux-gnu -y
```
#### Darwin
```
brew install scons gcc-arm-embedded
```

### 安装rust交叉编译工具链
```
rustup target add armv7a-none-eabi
```

## 构建
使用全新构建
```shell
./x b --reconfigure --clean
```
使用增量构建
```shell
./x b
```

## 部署
