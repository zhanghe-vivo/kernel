# -*- coding: utf-8 -*-
import sys

def main():
    with open('bsp/qemu-vexpress-a9/log.txt', 'r') as file:
        print("########## test log ##########")

        lines = file.readlines()
        for line in lines:
            print(line)

        last_line = lines[-1]
        if 'PASSED' not in last_line:
            raise Exception("单元测试不通过！")


if __name__ == '__main__':
    sys.exit(main())