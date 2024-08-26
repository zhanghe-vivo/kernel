// TODO use cxx to auto generate
/**
 * Spinlock
 */
#ifdef RT_USING_SMP
#include <cpuport.h> /* for spinlock from arch */
struct rt_spinlock
{
    rt_hw_spinlock_t lock;
#ifdef RT_DEBUGING_SPINLOCK
    void *owner;
#endif
};
#else
typedef rt_base_t rt_spinlock_t;
struct rt_spinlock
{
    rt_spinlock_t lock;
};
#endif

// some as object information type enum, ObjectInfoType
enum rt_object_class_type
{
    RT_Object_Class_Null   = 0x00,  /**< The object is not used. */
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
    RT_Object_Class_Static = 0x80   /**< The object is a static object. */
};

struct rt_object
{
#if RT_NAME_MAX > 0
    char        name[RT_NAME_MAX];                       /**< dynamic name of kernel object */
#else
    const char *name;                                    /**< static name of kernel object */
#endif /* RT_NAME_MAX > 0 */
    rt_uint8_t  type;                                    /**< type of kernel object */
    rt_uint8_t  flag;                                    /**< flag of kernel object */

    rt_list_t   list;                                    /**< list node of kernel object */
};

typedef struct rt_object *rt_object_t;                   /**< Type for kernel objects. */

struct rt_object_information
{
    struct rt_spinlock        spinlock;
    rt_list_t                 object_list;              /**< object list */
    rt_size_t                 object_size;              /**< object size */
    enum rt_object_class_type type;                     /**< object class type */
};
