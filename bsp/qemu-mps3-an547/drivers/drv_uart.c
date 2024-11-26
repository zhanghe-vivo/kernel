#include <rthw.h>
#include <rtthread.h>
#include <rtdevice.h>
#include "board.h"

#include "SSE300MPS3.h"
#include "device_definition.h"
#include "uart_cmsdk_reg_map.h"

/* CTRL Register */
#define UART_CMSDK_TX_EN       (1ul << 0)
#define UART_CMSDK_RX_EN       (1ul << 1)
#define UART_CMSDK_TX_INTR_EN  (1ul << 2)
#define UART_CMSDK_RX_INTR_EN  (1ul << 3)

/* STATE Register */
#define UART_CMSDK_TX_BF  (1ul << 0)
#define UART_CMSDK_RX_BF  (1ul << 1)
#define UART_CMSDK_TX_BO  (1ul << 2)
#define UART_CMSDK_RX_BO  (1ul << 3)

/* INTSTATUS Register */
#define UART_CMSDK_TX_INTR  (1ul << 0)
#define UART_CMSDK_RX_INTR  (1ul << 1)

/* UART state definitions */
#define UART_CMSDK_INITIALIZED  (1ul << 0)

enum
{
#ifdef BSP_USING_UART0
    UART0_INDEX,
#endif
#ifdef BSP_USING_UART1
    UART1_INDEX,
#endif
};
/* qemu uart dirver class */
struct uart_instance
{
    const char *name;
    struct uart_cmsdk_reg_map_t *handle;
    IRQn_Type irq_num;
    int uart_index;
    struct rt_serial_device serial;
};
#if defined(BSP_USING_UART0)
#ifndef UART0_CONFIG
#define UART0_CONFIG                                                        \
    {                                                                       \
        .name = "uart0",                                                    \
        .handle = (struct uart_cmsdk_reg_map_t *)UART0_BASE_S,              \
        .irq_num = UARTRX0_IRQn,                                            \
        .uart_index = UART0_INDEX,                                          \
    }
#endif /* UART0_CONFIG */
#endif /* BSP_USING_UART0 */
#if defined(BSP_USING_UART1)
#ifndef UART1_CONFIG
#define UART1_CONFIG                                                        \
    {                                                                       \
        .name = "uart1",                                                    \
        .handle = (struct uart_cmsdk_reg_map_t *)UART1_BASE_S,              \
        .irq_num = UARTRX1_IRQn,                                            \
        .uart_index = UART1_INDEX,                                          \
    }
#endif /* UART1_CONFIG */
#endif /* BSP_USING_UART1 */

static struct uart_instance uart_obj[] =
{
#ifdef BSP_USING_UART0
    UART0_CONFIG,
#endif
#ifdef BSP_USING_UART1
    UART1_CONFIG,
#endif
};
static void uart_isr(struct rt_serial_device *serial)
{
    /* UART in mode Receiver */
    rt_hw_serial_isr(serial, RT_SERIAL_EVENT_RX_IND);
}
void UARTRX0_Handler(void)
{
#ifdef BSP_USING_UART0
    uint32_t irq_status = 0x00;
    /* enter interrupt */
    rt_interrupt_enter();
    uart_isr(&(uart_obj[UART0_INDEX].serial));
    irq_status = uart_obj[UART0_INDEX].handle->intr_reg.intrstatus;
    uart_obj[UART0_INDEX].handle->intr_reg.intrclear = irq_status;
    /* leave interrupt */
    rt_interrupt_leave();
#endif
}
void UARTRX1_Handler(void)
{
#ifdef BSP_USING_UART1
    uint32_t irq_status = 0x00;
    /* enter interrupt */
    rt_interrupt_enter();
    uart_isr(&(uart_obj[UART1_INDEX].serial));
    irq_status = uart_obj[UART1_INDEX].handle->intr_reg.intrstatus;
    uart_obj[UART1_INDEX].handle->intr_reg.intrclear = irq_status;
    /* leave interrupt */
    rt_interrupt_leave();
#endif
}
static rt_err_t uart_configure(struct rt_serial_device *serial, struct serial_configure *cfg)
{
    struct uart_instance *instance;
    RT_ASSERT(serial != RT_NULL);
    instance = (struct uart_instance *)serial->parent.user_data;
    uart_obj[instance->uart_index].handle->bauddiv = 16;
    uart_obj[instance->uart_index].handle->ctrl = UART_CMSDK_RX_INTR_EN | UART_CMSDK_RX_EN | UART_CMSDK_TX_EN;
    NVIC_EnableIRQ(uart_obj[instance->uart_index].irq_num);
    uart_obj[instance->uart_index].handle->state = 0;
    return RT_EOK;
}
static rt_err_t uart_control(struct rt_serial_device *serial, int cmd, void *arg)
{
    struct uart_instance *instance;
    RT_ASSERT(serial != RT_NULL);
    instance = (struct uart_instance *)serial->parent.user_data;
    switch (cmd)
    {
    case RT_DEVICE_CTRL_CLR_INT:
        /* disable rx irq */
        instance->handle->ctrl &= ~UART_CMSDK_RX_INTR_EN;
        break;
    case RT_DEVICE_CTRL_SET_INT:
        /* enable rx irq */
        instance->handle->ctrl |= UART_CMSDK_RX_INTR_EN;
        break;
    }
    return RT_EOK;
}
static int uart_putc(struct rt_serial_device *serial, char c)
{
    struct uart_instance *instance;
    RT_ASSERT(serial != RT_NULL);
    instance = (struct uart_instance *)serial->parent.user_data;
    instance->handle->data = c;
    return 1;
}
static int uart_getc(struct rt_serial_device *serial)
{
    int ch;
    uint32_t state = 0;
    struct uart_instance *instance;
    RT_ASSERT(serial != RT_NULL);
    instance = (struct uart_instance *)serial->parent.user_data;
    ch = -1;
    if (!instance)
        return ch;
    state = instance->handle->state;
    if (state)
    {
        ch = instance->handle->data & 0xff;
        instance->handle->state = 0;
    }
    return ch;
}
static const struct rt_uart_ops _uart_ops =
{
    uart_configure,
    uart_control,
    uart_putc,
    uart_getc,
};
int rt_hw_uart_init(void)
{
    struct serial_configure config = RT_SERIAL_CONFIG_DEFAULT;
    rt_err_t result = 0;
    for (rt_size_t i = 0; i < sizeof(uart_obj) / sizeof(struct uart_instance); i++)
    {
        /* init UART object */
        uart_obj[i].serial.ops = &_uart_ops;
        uart_obj[i].serial.config = config;
        /* register UART device */
        result = rt_hw_serial_register(&uart_obj[i].serial, uart_obj[i].name,
                                       RT_DEVICE_FLAG_RDWR | RT_DEVICE_FLAG_INT_RX,
                                       &uart_obj[i]);
        RT_ASSERT(result == RT_EOK);
    }
    return result;
}
