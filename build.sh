#!/bin/bash


macros=""
parse_config(){
    config_file="rtconfig.h"
    while IFS= read -r line
    do
        if echo "$line" | grep -q "^#define"; then
            macro_name=$(echo "$line" | awk '{print $2}')
            macro_value=$(echo "$line" | awk '{print $3}')
            if ! [ -n "$macro_value" ]; then
                macros="$macros $macro_name"   
            fi
        fi
    done < "$config_file"
}

echo "请选择需要编译的目标平台"
echo "1. qemu-vexpress-a9"
echo "2. qemu-virt64-aarch64"
echo "3. qemu-virt64-riscv"
read choice

case $choice in
    1)
        echo "build qemu-vexpress-a9"
        path="qemu-vexpress-a9"
        target_toolchain="armv7a-none-eabi"
        ;;
    2)
        echo "build qemu-virt64-aarch64"
        path="qemu-virt64-aarch64"
        target_toolchain="aarch64-unknown-none"
        ;;
    *)
        echo "build qemu-virt64-riscv"
        path="qemu-virt64-riscv"
        target_toolchain="riscv64gc-unknown-none-elf"
        ;;
esac

cd bsp/$path
echo "是否重新配置编译选项？(y/n)"
read rebuild_config

if [ "$rebuild_config" = "y" ]; then
    scons --menuconfig
fi

parse_config

cd ../../rs_src

INCLUDE_PATH="../bsp/$path" cargo build --target $target_toolchain --features "$macros" 

cd ../bsp/$path

scons