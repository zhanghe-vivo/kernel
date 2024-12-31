#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import os
import sys
import os
import argparse

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..'))


def check_test_result(platform):
    log_path = os.path.join(ROOT, 'bsp', f'{platform}', 'log.txt')

    if not os.path.exists(log_path):
        raise FileNotFoundError(f"找不到日志文件：{log_path}")
    print(f"########## {platform} test log ##########")
    encodings = ['utf-8', 'latin-1']
    for encoding in encodings:
        try:
            with open(log_path, 'r', encoding=encoding) as file:
                    lines = file.readlines()
                    for line in lines:
                        print(line, end='')  # end='' 因为文件中的行已经包含换行符
                    last_line = lines[-1] if lines else ""
                    if 'PASSED' not in last_line:
                        raise Exception(f"平台 {platform} 的单元测试不通过！")
                    return
        except UnicodeDecodeError:
            continue
        raise Exception("Unable to decode file with any of the specified encodings.")
    



def main():
    parser = argparse.ArgumentParser(description='检查 QEMU 测试结果')
    parser.add_argument('platform', help='QEMU 平台名称 (例如: vexpress-a9)')

    args = parser.parse_args()

    try:
        check_test_result(args.platform)
        return 0
    except Exception as e:
        print(f"错误：{str(e)}", file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
