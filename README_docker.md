## Use docker
```bash
docker build -t compile_rtt .
docker images
docker run -d --user 0 -v "$PWD":/usr/src/rtt -w /usr/src/rtt compile_rtt tail -f /dev/null
docker exec -it compile_rtt zsh
```

## Use `colima` and `nerdctl`
```bash
brew install colima
colima start --arch aarch64 --vm-type=vz --vz-rosetta --cpu 2 --memory 4 --disk 60 --runtime containerd
colima nerdctl install
nerdctl build -t compile_rtt .
nerdctl images
nerdctl run -d -v "$PWD":/usr/src/rtt -w /usr/src/rtt compile_rtt tail -f /dev/null
nerdctl exec -it compile_rtt zsh
```

## Compile and run in docker
```bash
./build.sh qemu-vexpress-a9
cd basp/qemu-vexpress-a9
./qemu-nographic.sh
```
