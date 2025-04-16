//! vfs_node.rs  
#![allow(dead_code)]

use crate::{
    error::{code, Error},
    vfs::{vfs_log::*, vfs_path::*, vfs_traits::*},
};
use alloc::{
    collections::{BTreeMap, VecDeque},
    format,
    string::String,
    sync::Arc,
    vec::Vec,
};
use spin::{Mutex, RwLock};

/// Filesystem inode number
pub type InodeNo = u64;

/// File type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Regular,     // Regular file
    Directory,   // Directory
    CharDevice,  // Character device
    BlockDevice, // Block device
    SymLink,     // Symbolic link
}

/// Inode attributes
#[derive(Debug, Clone)]
pub struct InodeAttr {
    pub inode_no: InodeNo,   // Index Node number
    pub size: usize,         // File size
    pub blocks: u64,         // Number of blocks
    pub atime: u64,          // Access time
    pub mtime: u64,          // Modification time
    pub ctime: u64,          // Creation time
    pub file_type: FileType, // File type
    pub mode: u32,           // Access permissions
    pub nlinks: u32,         // Number of hard links
    pub uid: u32,            // User ID
    pub gid: u32,            // Group ID
}

impl InodeAttr {
    /// Create new inode attributes
    pub fn new(inode_no: InodeNo, file_type: FileType, mode: u32) -> Self {
        let now = 0; // TODO: Get current timestamp
        Self {
            inode_no,
            size: 0,
            blocks: 0,
            atime: now,
            mtime: now,
            ctime: now,
            file_type,
            mode,
            nlinks: 1,
            uid: 0,
            gid: 0,
        }
    }

    /// Check if it's a directory
    pub fn is_dir(&self) -> bool {
        self.file_type == FileType::Directory
    }

    /// Update access time
    pub fn update_atime(&mut self) {
        self.atime = 0; // TODO: Get current timestamp
    }

    /// Update modification time
    pub fn update_mtime(&mut self) {
        self.mtime = 0; // TODO: Get current timestamp
    }
}

/// VFS inode
pub struct Inode {
    /// Inode attributes
    pub attr: RwLock<InodeAttr>,

    /// Parent filesystem
    pub fs_ops: Arc<dyn VfsOperations>,

    /// Reference count
    ref_count: RwLock<usize>,
}

impl Inode {
    /// Create new inode
    pub fn new(attr: InodeAttr, fs_ops: Arc<dyn VfsOperations>) -> Self {
        Self {
            attr: RwLock::new(attr),
            fs_ops,
            ref_count: RwLock::new(1),
        }
    }

    /// Increment reference count
    pub fn inc_ref(&self) {
        let mut count = self.ref_count.write();
        *count += 1;
    }

    /// Decrement reference count, return new count
    pub fn dec_ref(&self) -> usize {
        let mut count = self.ref_count.write();
        *count -= 1;
        *count
    }

    /// Get reference count
    pub fn ref_count(&self) -> usize {
        *self.ref_count.read()
    }

    /// Get inode number
    pub fn inode_no(&self) -> InodeNo {
        self.attr.read().inode_no
    }

    /// Get file type
    pub fn file_type(&self) -> FileType {
        self.attr.read().file_type
    }

    /// Check if it's a directory
    pub fn is_dir(&self) -> bool {
        self.file_type() == FileType::Directory
    }

    /// Check if it's a regular file
    pub fn is_file(&self) -> bool {
        self.file_type() == FileType::Regular
    }

    /// Check if it's a symbolic link
    pub fn is_symlink(&self) -> bool {
        self.file_type() == FileType::SymLink
    }
}

/// Directory node
pub struct DNode {
    /// Parent directory
    parent: Option<Arc<DNode>>,

    /// Child directories list (name -> DNode)
    children: RwLock<BTreeMap<String, Arc<DNode>>>,

    /// Associated inode
    inode: Arc<Inode>,

    /// Directory entry name
    name: String,

    /// Reference count
    ref_count: RwLock<usize>,
}

impl DNode {
    /// Create new directory node
    pub fn new(name: String, inode: Arc<Inode>, parent: Option<Arc<DNode>>) -> Self {
        Self {
            parent,
            children: RwLock::new(BTreeMap::new()),
            inode,
            name,
            ref_count: RwLock::new(1),
        }
    }

    /// Increment reference count
    pub fn inc_ref(&self) {
        let mut count = self.ref_count.write();
        *count += 1;
        // Also increment associated inode reference count
        self.inode.inc_ref();
    }

    /// Decrement reference count, return new count
    pub fn dec_ref(&self) -> usize {
        let mut count = self.ref_count.write();
        *count -= 1;
        // Also decrement associated inode reference count
        let inode_count = self.inode.dec_ref();
        // If inode reference count is 0, notify filesystem to free inode
        if inode_count == 0 {
            let _ = self.inode.fs_ops.free_inode(self.inode.inode_no());
        }
        *count
    }

    /// Check if directory is empty
    pub fn is_empty(&self) -> bool {
        let children = self.children.read();

        // Check if there are any children
        if children.is_empty() {
            return true;
        }

        // Consider empty if only contains "." and ".."
        children.iter().all(|(name, _)| name == "." || name == "..")
    }

    /// Add child node
    pub fn add_child(&self, name: String, child: Arc<DNode>) {
        self.children.write().insert(name, child);
    }

    /// Remove child node
    pub fn remove_child(&self, name: &str) -> Option<Arc<DNode>> {
        self.children.write().remove(name)
    }

    /// Find child node
    pub fn find_child(&self, name: &str) -> Option<Arc<DNode>> {
        self.children.read().get(name).cloned()
    }

    /// Get full path
    pub fn get_full_path(&self) -> String {
        if self.name == "/" {
            return String::from("/");
        }

        let mut path = String::new();
        let mut current = Some(self);

        while let Some(node) = current {
            if !node.name.is_empty() {
                path = format!("/{}{}", node.name, path);
            }
            current = node.parent.as_ref().map(|p| p.as_ref());
        }

        if path.is_empty() {
            path.push('/');
        }
        path
    }

    /// Get associated inode
    pub fn get_inode(&self) -> Arc<Inode> {
        self.inode.clone()
    }

    /// Get parent node
    pub fn get_parent(&self) -> Option<Arc<DNode>> {
        self.parent.clone()
    }
}

/// Cache entry
struct CacheEntry {
    dnode: Arc<DNode>,
    hash: u32,
    full_path: String,
    access_time: u64,
}

impl CacheEntry {
    fn new(dnode: Arc<DNode>, hash: u32) -> Self {
        Self {
            full_path: dnode.get_full_path(),
            dnode,
            hash,
            access_time: 0, // TODO: Get current timestamp
        }
    }
}

/// Hash table bucket
struct Bucket {
    entries: Mutex<Vec<CacheEntry>>,
}

impl Bucket {
    fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }
}

/// DJB2 hash algorithm variant, string hash function
fn cal_hash(s: &str) -> u32 {
    let mut hash: u32 = 5381;
    for c in s.bytes() {
        // hash = ((hash << 5) + hash) + c
        hash = hash
            .wrapping_shl(5)
            .wrapping_add(hash)
            .wrapping_add(c as u32);
    }
    hash
}

/// Directory node cache
pub struct DNodeCache {
    /// Hash bucket array
    buckets: Vec<Bucket>,

    /// LRU queue, stores hash values
    lru_list: Mutex<VecDeque<u32>>,

    /// Cache capacity
    capacity: usize,

    /// Number of buckets
    bucket_count: usize,
}

impl DNodeCache {
    pub fn new(capacity: usize, bucket_count: usize) -> Self {
        let mut buckets = Vec::with_capacity(bucket_count);
        for _ in 0..bucket_count {
            buckets.push(Bucket::new());
        }

        let cache = Self {
            buckets,
            lru_list: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
            bucket_count,
        };

        vfslog!("[vfs_node] DNodeCache initialized successfully");
        cache
    }

    /// Calculate path hash
    fn hash_path(&self, path: &str) -> u32 {
        // Ensure using normalized path for hash calculation
        let normalized_path = normalize_path(path).unwrap_or_else(|| String::from("/"));
        let hash = cal_hash(&normalized_path);

        hash
    }

    /// Get bucket index
    fn get_bucket_index(&self, hash: u32) -> usize {
        let index = (hash as usize) % self.bucket_count;
        index
    }

    /// Update LRU list
    fn update_lru(&self, hash: u32) {
        let mut lru = self.lru_list.lock();

        // Find and remove old position
        if let Some(index) = lru.iter().position(|&x| x == hash) {
            lru.remove(index);
        }

        // Add to queue tail
        lru.push_back(hash);

        // If LRU list exceeds capacity, remove oldest item
        if lru.len() > self.capacity {
            if let Some(old_hash) = lru.pop_front() {
                vfslog!(
                    "[vfs_node] LRU list exceeded capacity, removed oldest hash {}",
                    old_hash
                );
            }
        }
    }

    /// Lookup directory node
    pub fn lookup(&self, path: &str) -> Option<Arc<DNode>> {
        let normalized_path = normalize_path(path)?;
        let hash = self.hash_path(&normalized_path);
        let bucket_idx = self.get_bucket_index(hash);
        let bucket = &self.buckets[bucket_idx];
        let entries = bucket.entries.lock();

        for entry in entries.iter() {
            if entry.hash == hash && entry.full_path == normalized_path {
                self.update_lru(hash);
                return Some(entry.dnode.clone());
            }
        }

        None
    }

    /// Insert directory node into cache
    pub fn insert(&self, dnode: Arc<DNode>) {
        let path = dnode.get_full_path();
        // Use vfs_path's normalize_path
        let normalized_path = normalize_path(&path).unwrap_or_else(|| String::from("/"));
        let hash = self.hash_path(&normalized_path);
        let bucket_idx = self.get_bucket_index(hash);

        let bucket = &self.buckets[bucket_idx];
        let mut entries = bucket.entries.lock();

        // Check if already exists
        if entries
            .iter()
            .any(|e| e.hash == hash && e.full_path == normalized_path)
        {
            return;
        }

        // Check capacity
        if entries.len() >= self.capacity {
            let mut lru = self.lru_list.lock();
            if let Some(old_hash) = lru.pop_front() {
                let old_bucket_idx = self.get_bucket_index(old_hash);
                let old_bucket = &self.buckets[old_bucket_idx];
                let mut old_entries = old_bucket.entries.lock();
                if let Some(pos) = old_entries.iter().position(|e| e.hash == old_hash) {
                    old_entries.remove(pos);
                }
            }
        }

        // Create new cache entry
        let entry = CacheEntry {
            dnode: dnode.clone(),
            hash,
            full_path: normalized_path,
            access_time: 0,
        };

        entries.push(entry);
        self.lru_list.lock().push_back(hash);
    }

    /// Remove directory node from cache
    pub fn remove(&self, path: &str) -> Option<Arc<DNode>> {
        // Normalize path
        let normalized_path = normalize_path(path)?;
        let hash = self.hash_path(&normalized_path);
        let bucket_idx = self.get_bucket_index(hash);

        vfslog!(
            "[vfs_node] Cache remove: path = {}, normalized = {}, hash = {}, bucket = {}",
            path,
            normalized_path,
            hash,
            bucket_idx
        );

        let bucket = &self.buckets[bucket_idx];
        let mut entries = bucket.entries.lock();

        if let Some(pos) = entries
            .iter()
            .position(|e| e.hash == hash && e.full_path == normalized_path)
        {
            let entry = entries.remove(pos);
            vfslog!("[vfs_node] Removed entry from cache: {}", normalized_path);

            // Remove from LRU list
            let mut lru = self.lru_list.lock();
            if let Some(index) = lru.iter().position(|&x| x == hash) {
                lru.remove(index);
                vfslog!("[vfs_node] Removed hash {} from LRU list", hash);
            }

            Some(entry.dnode)
        } else {
            vfslog!("[vfs_node] Entry not found in cache: {}", normalized_path);
            None
        }
    }

    /// Clear cache
    pub fn clear(&self) {
        vfslog!("[vfs_node] Clearing entire cache");

        let mut total_entries = 0;
        for (i, bucket) in self.buckets.iter().enumerate() {
            let mut entries = bucket.entries.lock();
            total_entries += entries.len();
            entries.clear();
            vfslog!("[vfs_node] Cleared bucket {}", i);
        }

        self.lru_list.lock().clear();
        vfslog!(
            "[vfs_node] Cleared cache: removed {} entries from {} buckets",
            total_entries,
            self.buckets.len()
        );
    }
}

/// Global directory node cache instance
static DNODE_CACHE: RwLock<Option<DNodeCache>> = RwLock::new(None);
/// Default cache capacity (max items per bucket)
const DNODE_CACHE_CAPACITY: usize = 1024;
/// Default hash bucket count
const DNODE_CACHE_BUCKETS: usize = 256;

/// Initialize directory node cache
pub fn init_dnode_cache() -> Result<(), Error> {
    let mut cache = DNODE_CACHE.write();
    if cache.is_some() {
        return Err(code::EEXIST);
    }
    *cache = Some(DNodeCache::new(DNODE_CACHE_CAPACITY, DNODE_CACHE_BUCKETS));
    Ok(())
}

/// Get directory node cache instance
pub fn get_dnode_cache() -> Option<&'static DNodeCache> {
    unsafe { DNODE_CACHE.read().as_ref().map(|c| &*(c as *const _)) }
}

/// Clean directory node cache
pub fn cleanup_dnode_cache() {
    let mut cache = DNODE_CACHE.write();
    if let Some(c) = cache.as_ref() {
        c.clear();
    }
    *cache = None;
}
