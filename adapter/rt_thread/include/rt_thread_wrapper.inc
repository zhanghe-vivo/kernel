typedef void (*TimeoutFn)(void *);
enum rt_object_class_type
{
    RT_Object_Class_Null   = 0,  /**< The object is not used. */
    RT_Object_Class_Process = 1,         /**< The object is a process. */
    RT_Object_Class_Thread,         /**< The object is a thread. */
#ifdef RT_USING_SEMAPHORE
    RT_Object_Class_Semaphore,      /**< The object is a semaphore. */
#endif
#ifdef RT_USING_MUTEX
    RT_Object_Class_Mutex,          /**< The object is a mutex. */
#endif
#ifdef RT_USING_EVENT
    RT_Object_Class_Event,          /**< The object is a event. */
#endif
#ifdef RT_USING_MAILBOX
    RT_Object_Class_MailBox,        /**< The object is a mail box. */
#endif
#ifdef RT_USING_MESSAGEQUEUE
    RT_Object_Class_MessageQueue,   /**< The object is a message queue. */
#endif
#ifdef RT_USING_MEMHEAP
    RT_Object_Class_MemHeap,        /**< The object is a memory heap. */
#endif
#ifdef RT_USING_MEMPOOL
    RT_Object_Class_MemPool,        /**< The object is a memory pool. */
#endif
#ifdef RT_USING_DEVICE
    RT_Object_Class_Device,         /**< The object is a device. */
#endif
    RT_Object_Class_Timer,          /**< The object is a timer. */
#ifdef RT_USING_MODULE
    RT_Object_Class_Module,         /**< The object is a module. */
#endif
#ifdef RT_USING_HEAP
    RT_Object_Class_Memory,         /**< The object is a memory. */
#endif
#ifdef RT_USING_SMART
    RT_Object_Class_Channel,        /**< The object is a channel */
#endif
#ifdef RT_USING_HEAP
    RT_Object_Class_Custom,         /**< The object is a custom object */
#endif
    RT_Object_Class_Unknown,        /**< The object is unknown. */
};
#ifdef RT_USING_SMP
struct tickets {
    rt_size_t owner;
    rt_size_t next;
};

struct rt_spinlock
{
    rt_size_t lock;
    tickets ticket;
#ifdef RT_DEBUGING_SPINLOCK
    void *owner;
#endif
};
#else
struct rt_spinlock{
    rt_size_t lock;
};
#endif
#define RawSpin rt_spinlock
#define int8_t rt_int8_t
#define uint8_t rt_uint8_t
#define int16_t rt_int16_t
#define uint16_t rt_uint16_t
#define int32_t rt_int32_t
#define uint32_t rt_uint32_t
#define int64_t rt_int64_t
#define uint64_t rt_uint64_t
#define uintptr_t rt_size_t
#define intptr_t rt_ssize_t
#define bool rt_bool_t
#define LinkedListNode rt_list_t

#define NAME_MAX 8
struct rt_thread;
#include "rust_wrapper.inc"
typedef struct rt_object *rt_object_t;