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
    toml_path = os.path.join(ROOT, 'blue/Cargo.toml')
    out_path = os.path.join(ROOT, 'blue/target')
    bsp_path = os.path.join(ROOT, f'bsp/{target}')
    gcc_path = os.getenv('RTT_EXEC_PATH', os.path.join(ROOT, '/bin'))
    gcc_include_path = os.path.join(gcc_path, 'include')
    include_path = f'{bsp_path};{ROOT}/include;{ROOT}/components/finsh;{gcc_include_path}'
    compat_os = 'rt_thread'
    toolchain = toml['target'][config.target]['toolchain']
    if config.reconfigure:
        rc = subprocess.call(['scons', '--menuconfig'], cwd=bsp_path)
        if rc != 0:
            logging.error(cmd)
            return rc
    features = ParseRTConfig(config, toml)
    if config.clean:
        rc = subprocess.call(['scons', '--clean'], cwd=bsp_path)
        if rc != 0:
            logging.error(cmd)
            return rc
        action = 'clean'
        cmd = f'COMPAT_OS="{compat_os}" INCLUDE_PATH="{include_path}" cargo {action} --manifest-path {toml_path} --target {toolchain} -Z unstable-options'
        rc = subprocess.call(cmd, shell=True, cwd=os.path.join(ROOT, 'blue'))
        if rc != 0:
            logging.error(cmd)
            return rc
    if config.fix:
        action = 'fix'
        cmd = f'COMPAT_OS="{compat_os}" INCLUDE_PATH="{include_path}" cargo {action} --allow-dirty --allow-staged --bins --lib --manifest-path {toml_path} --target {toolchain} -Z unstable-options --features ' + shlex.quote(
            features)
        rc = subprocess.call(cmd, shell=True, cwd=os.path.join(ROOT, 'blue'))
        if rc != 0:
            logging.error(cmd)
            return rc
    if 'USE_RUST' in features:
        action = 'build'
        cmd = f'COMPAT_OS="{compat_os}" INCLUDE_PATH="{include_path}" cargo {action} --manifest-path {toml_path} --target {toolchain} --artifact-dir {out_path} -Z unstable-options --features ' + shlex.quote(
            features)
        rc = subprocess.call(cmd, shell=True, cwd=os.path.join(ROOT, 'blue'))
        if rc != 0:
            logging.error(cmd)
            return rc
    return subprocess.call(['scons'], cwd=bsp_path)


def Clippy(config, toml):
    target = config.target
    toml_path = os.path.join(ROOT, 'blue/Cargo.toml')
    out_path = os.path.join(ROOT, 'blue/target')
    bsp_path = os.path.join(ROOT, f'bsp/{target}')
    gcc_path = os.getenv('RTT_EXEC_PATH', os.path.join(ROOT, '/bin'))
    gcc_include_path = os.path.join(gcc_path, 'include')
    include_path = f'{bsp_path};{ROOT}/include;{ROOT}/components/finsh;{gcc_include_path}'
    compat_os = 'rt_thread'
    features = ParseRTConfig(config, toml)
    toolchain = toml['target'][config.target]['toolchain']
    cmd = f'cargo clippy --manifest-path {toml_path}'
    cmd = f'COMPAT_OS="{compat_os}" INCLUDE_PATH="{include_path}" cargo clippy --manifest-path {toml_path} --target {toolchain} --features ' + shlex.quote(
        features) + ' -- -D clippy::undocumented_unsafe_blocks'
    return subprocess.call(cmd, shell=True, cwd=os.path.join(ROOT, 'blue'))


def Clean(config, toml):
    # TODO(Kai Luo)
    pass


def Format(config, toml):
    # TODO(Kai Luo): Use clang-format-diff.py to format C/C++/Java/JavaScript code.
    return subprocess.call(['cargo', 'fmt'], cwd=os.path.join(ROOT, 'blue'))


def ListTargets(config, toml):
    for t in toml['target']:
        print(t)


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
    build.add_argument('--fix',
                       action='store_true',
                       default=False,
                       help=u"构建前修复代码")
    build.add_argument('--clean',
                       action='store_true',
                       default=False,
                       help=u"构建前清除")
    build.set_defaults(func=Build)
    clean = subparsers.add_parser('clean', aliases=['c'], description=u"清除")
    clean.set_defaults(func=Clean)
    fmt = subparsers.add_parser('format', aliases=['f'], description=u"格式化代码")
    fmt.set_defaults(func=Format)
    lst = subparsers.add_parser('list', aliases=['l'], description=u"列出支持的目标")
    lst.set_defaults(func=ListTargets)
    clippy = subparsers.add_parser('clippy', description=u"代码规范检测")
    clippy.set_defaults(func=Clippy)
    clippy.add_argument('--target',
                        default='qemu-vexpress-a9',
                        help=u"指定构建的目标平台")
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
