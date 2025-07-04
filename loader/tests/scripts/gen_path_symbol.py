#!/usr/bin/env python3
# -*- coding: utf-8 -*-
# Copyright (c) 2025 vivo Mobile Communication Co., Ltd.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#       http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

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
