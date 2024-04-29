## use docker
```bash
sudo docker build -t compile_rtt -f Dockerfile .
sudo docker images
sudo docker run -d --user "$(id -u)":"$(id -g)" -v "$PWD":/usr/src/rtt -w /usr/src/rtt compile_rtt tail -f /dev/null
sudo docker exec -it compile_rtt bash
```

## use containerd and nerdctl
https://github.com/containerd/nerdctl

```bash
nerdctl build -t compile_rtt .
nerdctl images
nerdctl run -d --user "$(id -u)":"$(id -g)" -v "$PWD":/usr/src/rtt -w /usr/src/rtt compile_rtt tail -f /dev/null
nerdctl exec -it compile_rtt bash
```

## compile and run in docker
```bash
./build.sh qemu-vexpress-a9
cd basp/qemu-vexpress-a9
./qemu-nographic.sh
```