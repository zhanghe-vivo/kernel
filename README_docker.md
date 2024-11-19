## Use docker
```bash
sudo docker build -t compile_rtt .
sudo docker images
sudo docker run -d --user "$(id -u)":"$(id -g)" -v "$PWD":/usr/src/rtt -w /usr/src/rtt compile_rtt tail -f /dev/null
sudo docker exec -it compile_rtt bash
```

## Use `colima` and `nerdctl`
```bash
brew install colima
colima start --arch aarch64 --vm-type=vz --vz-rosetta --cpu 2 --memory 4 --disk 60 --runtime containerd
colima nerdctl install
nerdctl build -t compile_rtt .
nerdctl images
nerdctl run -d -v "$PWD":/usr/src/rtt -w /usr/src/rtt compile_rtt tail -f /dev/null
nerdctl exec -it compile_rtt bash
```

## Compile and run in docker
```bash
./build.sh qemu-vexpress-a9
cd basp/qemu-vexpress-a9
./qemu-nographic.sh
```
