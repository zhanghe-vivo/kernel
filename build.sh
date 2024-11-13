#!/bin/bash

project_path=$(pwd)

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
    echo "2. qemu-mps2-an385"
    echo "3. qemu-mps3-an547"
    echo "4. qemu-virt64-aarch64"
    echo "5. qemu-virt64-riscv"
    
    read choice

# https://docs.rust-embedded.org/book/intro/install.html
    case $choice in
        1)
            echo "build qemu-vexpress-a9"
            bsp_path="qemu-vexpress-a9"
            target_toolchain="armv7a-none-eabi"
            include_path="$project_path/bsp/$bsp_path;$project_path/include;$project_path/components/drivers/include;$project_path/components/finsh;$project_path/components/legacy;$project_path/libcpu/arm/cortex-a"
            ;;
        2)
            echo "build qemu-mps2-an385"
            bsp_path="qemu-mps2-an385"
            target_toolchain="thumbv7m-none-eabi"
            include_path="$project_path/bsp/$bsp_path;$project_path/include;$project_path/components/drivers/include;$project_path/components/finsh;$project_path/components/legacy;$project_path/libcpu/arm/cortex-m3"
            ;;
        3)
            echo "build qemu-mps3-an547"
            bsp_path="qemu-mps3-an547"
            target_toolchain="thumbv8m.main-none-eabi"
            include_path="$project_path/bsp/$bsp_path;$project_path/include;$project_path/components/drivers/include;$project_path/components/finsh;$project_path/components/legacy;$project_path/libcpu/arm/cortex-m55"
            ;;
        4)
            echo "build qemu-virt64-aarch64"
            bsp_path="qemu-virt64-aarch64"
            target_toolchain="aarch64-unknown-none"
            include_path="$project_path/bsp/$bsp_path;$project_path/include;$project_path/components/drivers/include;$project_path/components/finsh;$project_path/components/legacy;$project_path/libcpu/aarch64/cortex-a"
            ;;
        *)
            echo "build qemu-virt64-riscv"
            bsp_path="qemu-virt64-riscv"
            target_toolchain="riscv64gc-unknown-none-elf"
            include_path="$project_path/bsp/$bsp_path;$project_path/include;$project_path/components/drivers/include;$project_path/components/finsh;$project_path/components/legacy;$project_path/libcpu/risc-v/virt64"
            ;;
    esac

    cd bsp/$bsp_path
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
        cd ../../blue
        cargo clean
        cd ../bsp/$bsp_path
    fi

    if [[ "$macros" == *"USE_RUST"* ]]; then
        cd ../../blue
        COMPAT_OS="rt_thread" INCLUDE_PATH="$include_path" cargo build --target $target_toolchain --features "$macros" 
        cd ../bsp/$bsp_path
    fi

    scons
else 

    target="$1"
    if [ "$target" = "qemu-vexpress-a9" ]; then
        bsp_path="qemu-vexpress-a9"
        target_toolchain="armv7a-none-eabi"
        include_path="$project_path/bsp/$bsp_path;$project_path/include;$project_path/components/drivers/include;$project_path/components/finsh;$project_path/components/legacy;$project_path/libcpu/arm/cortex-a"
    elif [ "$target" = "qemu-mps2-an385" ]; then
        bsp_path="qemu-mps2-an385"
        target_toolchain="thumbv7m-none-eabi"
        include_path="$project_path/bsp/$bsp_path;$project_path/include;$project_path/components/drivers/include;$project_path/components/finsh;$project_path/components/legacy;$project_path/libcpu/arm/cortex-m3"
    elif [ "$target" = "qemu-mps3-an547" ]; then
        bsp_path="qemu-mps3-an547"
        target_toolchain="thumbv8m.main-none-eabi"
        include_path="$project_path/bsp/$bsp_path;$project_path/include;$project_path/components/drivers/include;$project_path/components/finsh;$project_path/components/legacy;$project_path/libcpu/arm/cortex-m55"
    elif [ "$target" = "qemu-virt64-aarch64" ]; then
        bsp_path="qemu-virt64-aarch64"
        target_toolchain="aarch64-unknown-none"
        include_path="$project_path/bsp/$bsp_path;$project_path/include;$project_path/components/drivers/include;$project_path/components/finsh;$project_path/components/legacy;$project_path/libcpu/aarch64/cortex-a"
    elif [ "$target" = "qemu-virt64-riscv" ]; then
        bsp_path="qemu-virt64-riscv"
        target_toolchain="riscv64gc-unknown-none-elf"
        include_path="$project_path/bsp/$bsp_path;$project_path/include;$project_path/components/drivers/include;$project_path/components/finsh;$project_path/components/legacy;$project_path/libcpu/risc-v/virt64"
    fi

    cd bsp/$bsp_path
    scons --clean
    cd ../../blue
    cargo clean
    cargo fmt
    cd ../bsp/$bsp_path
    
    parse_config

    if [[ "$macros" == *"USE_RUST"* ]]; then
        cd ../../blue
        COMPAT_OS="rt_thread" INCLUDE_PATH="$include_path" cargo build --target $target_toolchain --features "$macros"
        cd ../bsp/$bsp_path
    fi
    scons
fi
