if [ ! -f "sd.bin" ]; then
dd if=/dev/zero of=sd.bin bs=1024 count=65536
fi


qemu-system-arm -M vexpress-a9 -smp cpus=1 -kernel rtthread.elf -nographic -sd sd.bin -S -s

# gdb-multiarch --tui rtthread.elf
# (gdb) target remote localhost:1234
