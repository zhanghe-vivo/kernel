#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Parse the int configuration item in Kconfig
Use the value of .config first, if not, use the default value
Generate config for rust build
"""

import sys
from kconfiglib import Kconfig, BOOL, STRING
import os
import argparse


def parse_rustflags(kconfig_path, board, build_type):
    rustflags = []
    kconf = Kconfig(kconfig_path)
    dotconfig = os.path.join(os.path.dirname(kconfig_path), board, build_type,
                             'defconfig')
    if os.path.exists(dotconfig):
        kconf.load_config(dotconfig)
    for sym in kconf.defined_syms:
        if sym.type == BOOL and sym.tri_value == 2:
            rustflags.append(sym.name.lower())
        elif sym.type == STRING and sym.str_value:
            rustflags.append(f'{sym.name.lower()}="{sym.str_value.lower()}"')
    return rustflags


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument("--kconfig", help="Kconfig dir")
    parser.add_argument("--board", help="target board")
    parser.add_argument("--build_type", help="target build_type")
    args = parser.parse_args()
    try:
        rustflags = parse_rustflags(args.kconfig, args.board, args.build_type)
        if rustflags:
            for flag in rustflags:
                print(flag)
    except Exception as e:
        print(f"\n[ERROR] Parse failed: {e}", file=sys.stderr)
        sys.exit(1)
