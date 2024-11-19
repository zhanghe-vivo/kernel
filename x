#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import os
import sys
import re
import subprocess
import shlex
import shutil
import tempfile
import logging
import argparse
import json

ROOT = os.path.abspath(os.path.dirname(__file__))


def ParseRTConfig(config, toml):
    target = config.target
    ret = subprocess.run([
        os.path.join(ROOT, 'scripts/parse_rtconfig_h'),
        os.path.join(ROOT, f'bsp/{target}/rtconfig.h'),
    ],
                         capture_output=True)
    return ret.stdout.rstrip().decode('utf-8')


def Build(config, toml):
    target = config.target
    bsp_path = os.path.join(ROOT, f'bsp/{target}')
    if config.reconfigure:
        rc = subprocess.call(['scons', '--menuconfig'], cwd=bsp_path)
        if rc != 0:
            logging.error(cmd)
            return rc
    if config.clean:
        rc = subprocess.call(['scons', '--clean'], cwd=bsp_path)
        if rc != 0:
            logging.error(cmd)
            return rc
    features = ParseRTConfig(config, toml)
    if 'USE_RUST' in features:
        toolchain = toml['target'][config.target]['toolchain']
        cmd = f'INCLUDE_PATH={bsp_path} cargo build --target {toolchain} --features ' + shlex.quote(
            features)
        rc = subprocess.call(cmd, shell=True, cwd=os.path.join(ROOT, 'blue'))
        if rc != 0:
            logging.error(cmd)
            return rc
    return subprocess.call(['scons'], cwd=bsp_path)


def Clean(config, toml):
    # TODO(Kai Luo)
    pass


def Format(config, toml):
    # TODO(Kai Luo): Use clang-format-diff.py to format C/C++/Java/JavaScript code.
    return subprocess.call(['cargo', 'fmt'], cwd=os.path.join(ROOT, 'blue'))


def main():
    parser = argparse.ArgumentParser(description=u"BlueOS构建器")
    subparsers = parser.add_subparsers(dest='action')
    # TODO(Kai Luo): 支持交互式生成config.json.
    # TODO(Kai Luo): 支持指定config.json路径.
    build = subparsers.add_parser('build', aliases=['b'], description=u"构建")
    build.add_argument('--target',
                       default='qemu-vexpress-a9',
                       help=u"指定构建的目标平台")
    build.add_argument('--reconfigure',
                       action='store_true',
                       default=False,
                       help=u"重新配置编译选项")
    build.add_argument('--clean',
                       action='store_true',
                       default=False,
                       help=u"构建前清除")
    build.set_defaults(func=Build)
    clean = subparsers.add_parser('clean', aliases=['c'], description=u"清除")
    clean.set_defaults(func=Clean)
    fmt = subparsers.add_parser('format', aliases=['f'], description=u"格式化代码")
    fmt.set_defaults(func=Format)
    config = parser.parse_args()
    if not config.action:
        parser.print_help()
        return -1
    # FIXME(Kai Luo): 当前CI的Python版本太低，不能用tomllib，暂时不用Rust生态里常用的TOML.
    toml_filename = os.path.join(ROOT, 'config.json')
    if not os.path.exists(toml_filename):
        toml_filename = os.path.join(ROOT, 'config.example.json')
    # config.example.json should be always in the repo.
    assert (os.path.exists(toml_filename))
    with open(toml_filename, 'rb') as f:
        toml = json.load(f)
    return config.func(config, toml)


if __name__ == '__main__':
    sys.exit(main())
