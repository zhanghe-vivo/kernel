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


def is_elf_file(filepath):
    """Checks if a file is an ELF file by examining its magic bytes.

    Args:
        filepath: The path to the file.

    Returns:
        True if the file is an ELF file, False otherwise.
    """
    try:
        with open(filepath, 'rb') as f:
            magic = f.read(4)
            return magic == b'\x7fELF'
    except FileNotFoundError:
        return False
    except Exception:
        return False


def gen_file(out, sym, path):
    with open(out, 'w') as f:
        f.write(f'const char *{sym}="{path}";')
    return 0


def main():
    exe = os.path.abspath(sys.argv[1])
    sym = sys.argv[2]
    out = os.path.abspath(sys.argv[3])
    if not is_elf_file(exe):
        return -1
    return gen_file(out, sym, exe)


if __name__ == '__main__':
    sys.exit(main())
