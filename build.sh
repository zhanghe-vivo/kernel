#!/bin/bash


macros=""
parse_config(){
    config_file="rtconfig.h"
    while IFS= read -r line
    do
        if echo "$line" | grep -q "^#define"; then
            macro_name=$(echo "$line" | awk '{print $2}')
            macro_value=$(echo "$line" | awk '{print $3}')
            
            # need RT_NAME_MAX as feature when bigger than 0
            if [ "$macro_name" = "RT_NAME_MAX" ] && [ "$macro_value" != "0" ]; then
                macros="$macros $macro_name"
            elif ! [ -n "$macro_value" ]; then
                macros="$macros $macro_name"   
            fi
        fi
    done < "$config_file"
}

if [ $# -eq 0 ]; then
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

    sleep 1 &
    if [ "$rebuild_config" = "y" ]; then
        scons --menuconfig
    fi
    wait

    parse_config

    echo "是否clean？(y/n)"
    read clean_config
    sleep 1 &
    if [ "$clean_config" = "y" ]; then
        scons --clean
        cd ../../rs_src
        cargo clean
        cd ../bsp/$path
    fi

    if [[ "$macros" == *"USE_RUST"* ]]; then
        cd ../../rs_src
        INCLUDE_PATH="../bsp/$path" cargo build --target $target_toolchain --features "$macros" 
        cd ../bsp/$path
    fi

    scons
else 

    target="$1"
    if [ "$target" = "qemu-vexpress-a9" ]; then
        path="qemu-vexpress-a9"
        target_toolchain="armv7a-none-eabi"
    elif [ "$target" = "qemu-virt64-aarch64" ]; then
        path="qemu-virt64-aarch64"
        target_toolchain="aarch64-unknown-none"
    elif [ "$target" = "qemu-virt64-riscv" ]; then
        path="qemu-virt64-riscv"
        target_toolchain="riscv64gc-unknown-none-elf"
    fi

    cd bsp/$path
    scons --clean
    cd ../../rs_src
    cargo clean
    cargo fmt
    cd ../bsp/$path
    
    parse_config

    if [[ "$macros" == *"USE_RUST"* ]]; then
        cd ../../rs_src
        INCLUDE_PATH="../bsp/$path" cargo build --target $target_toolchain --features "$macros" 
        cd ../bsp/$path
    fi
    scons
fi
