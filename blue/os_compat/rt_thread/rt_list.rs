use core::ptr;

impl rt_list_node {
    /// Initializes a list object
    pub const fn new() -> Self {
        rt_list_node {
            next: ptr::null_mut(),
            prev: ptr::null_mut(),
        }
    }

    /// Insert a node after a list
    pub unsafe fn insert_after(&mut self, node: *mut rt_list_node) {
        (*self.next).prev = node;
        (*node).next = self.next;
        self.next = node;
        (*node).prev = self;
    }

    /// Insert a node before a list
    pub unsafe fn insert_before(&mut self, node: *mut rt_list_node) {
        (*self.prev).next = node;
        (*node).prev = self.prev;
        self.prev = node;
        (*node).next = self;
    }

    /// Remove node from list
    pub unsafe fn remove(&mut self) {
        (*self.next).prev = self.prev;
        (*self.prev).next = self.next;
        self.next = self;
        self.prev = self;
    }

    /// Tests whether a list is empty
    pub fn is_empty(&self) -> bool {
        self.next == self as *const _ as *mut rt_list_node
    }

    /// Get the list length
    pub fn len(&self) -> usize {
        let mut len = 0;
        let mut p = self as *const _ as *mut rt_list_node;
        unsafe {
            while (*p).next != self as *const _ as *mut rt_list_node {
                p = (*p).next;
                len += 1;
            }
        }
        len
    }
}

/// init the rt_list status
#[macro_export]
macro_rules! rt_list_init {
    ($node_ptr:expr) => {
        (*$node_ptr).prev = $node_ptr;
        (*$node_ptr).next = $node_ptr;
    };
}

#[macro_export]
macro_rules! container_of {
    ($ptr:expr, $type:path, $field:ident) => {
        $ptr.cast::<u8>()
            .sub(core::mem::offset_of!($type, $field))
            .cast::<$type>()
    };
}

/// Get the struct for this entry
#[macro_export]
macro_rules! rt_list_entry {
    ($node:expr, $type:ty, $($f:tt)*) => {
        rt_bindings::container_of!($node, $type, $($f)*)
    };
}
/// Iterate over a list
#[macro_export]
macro_rules! rt_list_for_each {
    ($pos:ident, $head:expr, $code:block) => {
        let mut $pos = $head.next;
        while !core::ptr::eq($pos, $head) {
            $code

            // Process $pos
            unsafe { $pos = (*$pos).next};
        }
    };
}

/// Iterate over a list safe against removal of list entry
#[macro_export]
macro_rules! rt_list_for_each_safe {
    ($pos:ident, $n:ident, $head:expr, $code:block) => {
        let mut $pos = (*$head).next;
        let mut $n;
        while $pos != $head {
            $code
            // Process $pos
            $n = (*$pos).next;
            $pos = $n;
        }
    };
}

/// Iterate over list of given type
#[macro_export]
macro_rules! rt_list_for_each_entry {
    ($pos:ident, $head:expr, $member:ident, $code:block) => {
        let mut $pos = rt_list_entry((*$head).next, type_name(*$pos), $member);
        while &(*$pos).$member != $head {
            $code
            // Process $pos
            $pos = rt_list_entry((*$pos).$member.next, type_name(*$pos), $member);
        }
    };
}

/// Iterate over list of given type safe against removal of list entry
#[macro_export]
macro_rules! rt_list_for_each_entry_safe {
    ($pos:ident, $n:ident, $head:expr, $member:ident, $code:block) => {
        let mut $pos = rt_list_entry((*$head).next, type_name(*$pos), $member);
        let mut $n = rt_list_entry((*$pos).$member.next, type_name(*$pos), $member);
        while &(*$pos).$member != $head {
            $code
            // Process $pos
            $pos = $n;
            $n = rt_list_entry((*$n).$member.next, type_name(*$pos), $member);
        }
    };
}

/// Get the first element from a list
#[macro_export]
macro_rules! rt_list_first_entry {
    ($ptr:expr, $type:ty, $member:ident) => {
        $rt_list_entry!($ptr.next, $type, stringify!($member))
    };
}

// single list
impl rt_slist_node {
    pub const fn new() -> Self {
        rt_slist_node {
            next: ptr::null_mut(),
        }
    }

    pub fn append(&mut self, n: *mut rt_slist_node) {
        let mut node = self;
        while !node.next.is_null() {
            node = unsafe { node.next.as_mut() }.unwrap();
        }
        node.next = n;
        unsafe {
            (*n).next = ptr::null_mut();
        }
    }

    pub fn insert(&mut self, n: *mut rt_slist_node) {
        unsafe {
            (*n).next = self.next;
        }
        self.next = n;
    }

    pub fn len(&self) -> u32 {
        let mut len = 0;
        let mut list = self.next;
        while !list.is_null() {
            list = unsafe { (*list).next };
            len += 1;
        }
        len
    }

    pub fn remove(&mut self, n: *mut rt_slist_node) -> *mut rt_slist_node {
        let mut node = self as *mut rt_slist_node;
        while !(unsafe { *node }).next.is_null() && (unsafe { *node }).next != n {
            node = unsafe { (*((*node).next)).next };
        }
        if !(unsafe { *node }).next.is_null() {
            (unsafe { *node }).next = unsafe { (*((*node).next)).next };
        }
        self as *mut rt_slist_node
    }

    pub fn first(&self) -> *mut rt_slist_node {
        self.next
    }

    pub fn tail(&mut self) -> *mut rt_slist_node {
        let mut l = self;
        while !l.next.is_null() {
            l = unsafe { l.next.as_mut() }.unwrap();
        }
        l
    }

    pub fn next(&self) -> *mut rt_slist_node {
        self.next
    }

    pub fn isempty(&self) -> bool {
        self.next.is_null()
    }
}

#[macro_export]
macro_rules! rt_slist_entry {
    ($node:expr, $type:ty, $member:ident) => {
        $container_of!($node, $type, $member)
    };
}

#[macro_export]
macro_rules! rt_slist_for_each {
    ($pos:ident, $head:expr, $code:block) => {
        let mut $pos = $head.next;
        while !$pos.is_null() {
            $code
            $pos = (*$pos).next;
        }
    };
}

#[macro_export]
macro_rules! rt_slist_for_each_entry {
    ($pos:ident, $head:expr, $member:ident, $code:block) => {
        let mut $pos = $rt_slist_entry!($head.next, type_name(*$pos), $member);
        while !$pos.$member.is_null() {
            $code
            $pos = $rt_slist_entry!($pos.$member.next, type_name(*$pos), $member);
        }
    };
}

#[macro_export]
macro_rules! rt_slist_first_entry {
    ($ptr:expr, $type:ty, $member:ident, $code:block) => {
        $rt_slist_entry!($ptr.next, $type, $member)
    };
}

#[macro_export]
macro_rules! rt_slist_tail_entry {
    ($ptr:expr, $type:ty, $member:ident) => {
        $rt_slist_entry!($rt_slist_tail($ptr), $type, $member)
    };
}
