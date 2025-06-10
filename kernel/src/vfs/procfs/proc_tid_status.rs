use crate::{error::Error, process::Kprocess, thread::Thread, vfs::procfs::*};
use alloc::{format, string::String, vec::Vec};
use core::fmt::Write;

pub struct ProcTaskFileOp {
    data: Vec<u8>,
    pid: u64,
    tid: usize,
    thread_name: String,
}
use bluekernel_infra::list::doubly_linked_list::LinkedListNode;

impl ProcTaskFileOp {
    pub(crate) fn new(pid: u64, tid: usize, thread_name: &str) -> Self {
        Self {
            data: Vec::new(),
            pid,
            tid,
            thread_name: String::from(thread_name),
        }
    }
}

impl ProcNodeOperationTrait for ProcTaskFileOp {
    fn get_content(&self) -> Result<Vec<u8>, Error> {
        let mut result = String::with_capacity(64);
        crate::foreach!(
            node,
            list,
            crate::object::ObjectClassType::ObjectClassThread,
            {
                let thread: &Thread;
                let thread_name: &str;
                unsafe {
                    let kobject =
                        crate::list_head_entry!(node.as_ptr(), crate::object::KObjectBase, list);
                    thread = &*(crate::list_head_entry!(kobject, crate::thread::Thread, parent));
                    thread_name = thread.get_name().to_str().expect("CStr to str failed");
                }
                if self.pid == Kprocess::get_process().pid && self.tid == thread.tid {
                    writeln!(result, "{:<9} {}", "Name:", thread_name).unwrap();
                    writeln!(result, "{:<9} {}", "State:", thread.stat.to_str()).unwrap();
                    writeln!(result, "{:<9} {}", "Pid:", thread.tid).unwrap();
                    writeln!(
                        result,
                        "{:<9} {}",
                        "Priority:",
                        thread.priority.get_current()
                    )
                    .unwrap();
                    return Ok(result.as_bytes().to_vec());
                }
            }
        );
        Ok(result.as_bytes().to_vec())
    }

    fn set_content(&self, content: Vec<u8>) -> Result<(usize), Error> {
        Ok(0)
    }
}
