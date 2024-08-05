struct rt_stack {
    rt_ubase_t sp;
    rt_uint8_t* stack_addr;
    rt_size_t size;
};

// c struct for RtThread
struct rt_thread {
    struct rt_object parent;
    rt_list_t tlist;
    rt_uint8_t stat;
    rt_uint8_t sched_flag_ttmr_set;
    rt_uint8_t current_priority;
    rt_uint8_t init_priority;
    rt_uint8_t number;
    rt_uint8_t high_mask;
    rt_uint32_t number_mask;
    rt_tick_t init_tick;
    rt_tick_t remaining_tick;
    struct rt_timer thread_timer;
    //Stack stack
    rt_ubase_t sp;
    rt_uint8_t* stack_addr;
    rt_size_t stack_size;

    void* entry;
    void* parameter;
    void* cleanup;
#ifdef RT_USING_SMP
    rt_uint8_t bind_cpu;
    rt_uint8_t oncpu;
#endif

    rt_err_t error;
#ifdef RT_USING_MUTEX
    rt_list_t taken_object_list;
    rt_object_t pending_object;
#endif
#ifdef RT_USING_EVENT
    rt_uint32_t event_set;
    rt_uint8_t event_info;
#endif
};