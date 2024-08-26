struct rt_timer {
    struct rt_object parent;
    void (*timeout_func)(void *parameter);
    void *parameter;
    rt_tick_t init_tick;
    rt_tick_t timeout_tick;
    rt_list_t node;
    struct rt_spinlock timer_lock;
};