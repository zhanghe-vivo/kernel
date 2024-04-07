/*
 * Copyright (c) 2006-2020, RT-Thread Development Team
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * Change Logs:
 * Date           Author       Notes
 * 2020/12/31     Bernard      Add license info
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <msh.h>
#include <finsh.h>

int main(void)
{
    printf("Hello RT-Thread!\n");

    msh_exec("utest_run",11);

    return 0;
}
