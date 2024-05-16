// is this terrible? God yes, but it works

use core::{fmt::Debug, ptr::NonNull};

use alloc::{
    alloc::{alloc, dealloc, handle_alloc_error},
    boxed::Box,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};

use crate::{log_info, log_ok};

static mut ROOT_VFS: Vfs = Vfs::null();

#[allow(unused)]
pub struct Vfs {
    mount_point: Option<String>,
    next: Option<NonNull<Vfs>>,
    ops: Option<NonNull<dyn FsOps>>,
    // vnode_covered: Option<*const VNode>,
    flags: u32,
    block_size: u32,
    pub data: *mut u8,
}

unsafe impl Sync for Vfs {}

impl Vfs {
    const fn null() -> Self {
        return Vfs {
            mount_point: None,
            next: None,
            ops: None,
            // vnode_covered: None,
            flags: 0,
            block_size: 0,
            data: core::ptr::null_mut(),
        };
    }

    fn as_ptr(&self) -> *const Self {
        core::ptr::addr_of!(*self)
    }

    pub fn mount(&mut self, path: &str) {
        if self.ops.is_none() {
            panic!("FsOps is null");
        }

        let vfsp = self.as_ptr();

        unsafe { self.ops.unwrap().as_mut().mount(path, &mut self.data, vfsp) };
    }

    pub fn unmount(&mut self) {
        if self.ops.is_none() {
            panic!("FsOps is null");
        }

        unsafe { self.ops.unwrap().as_mut().unmount(self.as_ptr()) };
    }

    pub fn root(&mut self) -> VNode {
        if self.ops.is_none() {
            panic!("FsOps is null");
        }

        unsafe { self.ops.unwrap().as_mut().root(self.as_ptr()) }
    }

    pub fn statfs(&mut self) -> StatFs {
        if self.ops.is_none() {
            panic!("FsOps is null");
        }

        unsafe { self.ops.unwrap().as_mut().statfs(self.as_ptr()) }
    }

    pub fn sync(&mut self) {
        if self.ops.is_none() {
            panic!("FsOps is null");
        }

        unsafe { self.ops.unwrap().as_mut().sync(self.as_ptr()) };
    }

    pub fn fid(&mut self, path: &str) -> Option<FileId> {
        if self.ops.is_none() {
            panic!("FsOps is null");
        }

        unsafe { self.ops.unwrap().as_mut().fid(path, self.as_ptr()) }
    }

    pub fn vget(&mut self, fid: FileId) -> VNode {
        if self.ops.is_none() {
            panic!("FsOps is null");
        }

        unsafe { self.ops.unwrap().as_mut().vget(fid, self.as_ptr()) }
    }
}

pub trait FsOps {
    // yes, the vfsp was the best solution I could come up with
    fn mount(&mut self, path: &str, data: &mut *mut u8, vfsp: *const Vfs);
    fn unmount(&mut self, vfsp: *const Vfs);
    fn root(&mut self, vfsp: *const Vfs) -> VNode;
    fn statfs(&mut self, vfsp: *const Vfs) -> StatFs;
    fn sync(&mut self, vfsp: *const Vfs);
    fn fid(&mut self, path: &str, vfsp: *const Vfs) -> Option<FileId>;
    // idk how the fuck you're supposed to accomplish this
    // good luck I guess.
    fn vget(&mut self, fid: FileId, vfsp: *const Vfs) -> VNode;
}

#[allow(unused)]
pub struct FileId {
    len: u16,
    data: u8,
}

#[allow(unused)]
pub struct StatFs {
    typ: u32,
    block_size: u32,
    total_blocks: u32,
    free_blocks: u32,
    available_blocks: u32, // non-protected blocks
    files: u32,
    free_nodes: u32,
    fs_id: u32,
    _reserved: [u8; 7],
}

#[repr(u8)]
pub enum VNodeType {
    // Jury is out on this one
    NON = 0,
    Regular,
    Directory,
    Block,
    Character,
    Link,
    Socket,
    Bad,
}

pub struct VNode {
    // for internal use only
    relative_path: String,
    pub flags: u16,
    pub ref_count: u16,
    pub shared_lock_count: u16,
    pub exclusive_lock_count: u16,
    ops: NonNull<dyn VNodeOperations>,
    pub node_data: Option<NodeData>,
    pub parent_vfs: *const Vfs,
    pub typ: VNodeType,
    pub data: *mut u8,
}

impl VNode {
    pub fn new(ops: Box<dyn VNodeOperations>, file_typ: VNodeType, parent_vfs: *const Vfs) -> Self {
        return Self {
            relative_path: "".to_string(),
            flags: 0,
            ref_count: 0,
            shared_lock_count: 0,
            exclusive_lock_count: 0,
            ops: unsafe { NonNull::new_unchecked(Box::into_raw(ops)) },
            node_data: None,
            parent_vfs,
            typ: file_typ,
            data: core::ptr::null_mut(),
        };
    }

    pub fn as_ptr(&self) -> *const VNode {
        core::ptr::addr_of!(*self)
    }

    // Trait functions
    pub fn open(&mut self, f: u32, c: UserCred) -> Result<Arc<[u8]>, ()> {
        unsafe { self.ops.as_mut().open(f, c, self.as_ptr()) }
    }

    pub fn close(&mut self, f: u32, c: UserCred) {
        unsafe { self.ops.as_mut().close(f, c, self.as_ptr()) }
    }

    pub fn rdwr(&mut self, uiop: *const UIO, direction: IODirection, f: u32, c: UserCred) {
        unsafe { self.ops.as_mut().rdwr(uiop, direction, f, c, self.as_ptr()) }
    }

    pub fn ioctl(&mut self, com: u32, d: *mut u8, f: u32, c: UserCred) {
        unsafe { self.ops.as_mut().ioctl(com, d, f, c, self.as_ptr()) }
    }

    pub fn select(&mut self, w: IODirection, c: UserCred) {
        unsafe { self.ops.as_mut().select(w, c, self.as_ptr()) }
    }

    pub fn getattr(&mut self, c: UserCred) -> VAttr {
        unsafe { self.ops.as_mut().getattr(c, self.as_ptr()) }
    }

    pub fn setattr(&mut self, va: VAttr, c: UserCred) {
        unsafe { self.ops.as_mut().setattr(va, c, self.as_ptr()) }
    }

    pub fn access(&mut self, m: u32, c: UserCred) {
        unsafe { self.ops.as_mut().access(m, c, self.as_ptr()) }
    }

    pub fn lookup(&mut self, nm: &str, c: UserCred) -> Result<VNode, ()> {
        let mut vnode = unsafe { self.ops.as_mut().lookup(nm, c, self.as_ptr()) }?;

        // TODO: the memory cost of this is pretty bad
        vnode.relative_path = self.relative_path.clone() + "/" + nm;

        unsafe {
            if let Some(mut new_vfs) = vfs_has_mount_point(
                &((*self.parent_vfs).mount_point.clone().unwrap() + &vnode.relative_path),
            ) {
                return Ok(new_vfs.as_mut().root());
            }
        }

        return Ok(vnode);
    }

    pub fn create(
        &mut self,
        nm: &str,
        va: VAttr,
        e: u32,
        m: u32,
        c: UserCred,
    ) -> Result<VNode, ()> {
        unsafe { self.ops.as_mut().create(nm, va, e, m, c, self.as_ptr()) }
    }

    pub fn link(&mut self, target_dir: *mut VNode, target_name: &str, c: UserCred) {
        unsafe {
            self.ops
                .as_mut()
                .link(target_dir, target_name, c, self.as_ptr())
        }
    }

    pub fn rename(&mut self, nm: &str, target_dir: *mut VNode, target_name: &str, c: UserCred) {
        unsafe {
            self.ops
                .as_mut()
                .rename(nm, target_dir, target_name, c, self.as_ptr())
        }
    }

    pub fn mkdir(&mut self, nm: &str, va: VAttr, c: UserCred) -> Result<VNode, ()> {
        unsafe { self.ops.as_mut().mkdir(nm, va, c, self.as_ptr()) }
    }

    pub fn readdir(&mut self, uiop: *const UIO, c: UserCred) {
        unsafe { self.ops.as_mut().readdir(uiop, c, self.as_ptr()) }
    }

    pub fn symlink(&mut self, link_name: &str, va: VAttr, target_name: &str, c: UserCred) {
        unsafe {
            self.ops
                .as_mut()
                .symlink(link_name, va, target_name, c, self.as_ptr())
        }
    }

    pub fn readlink(&mut self, uiop: *const UIO, c: UserCred) {
        unsafe { self.ops.as_mut().readlink(uiop, c, self.as_ptr()) }
    }

    pub fn fsync(&mut self, c: UserCred) {
        unsafe { self.ops.as_mut().fsync(c, self.as_ptr()) }
    }

    pub fn inactive(&mut self, c: UserCred) {
        unsafe { self.ops.as_mut().inactive(c, self.as_ptr()) }
    }

    pub fn bmap(&mut self, block_number: u32, bnp: ()) -> VNode {
        unsafe { self.ops.as_mut().bmap(block_number, bnp, self.as_ptr()) }
    }

    pub fn strategy(&mut self, bp: ()) {
        unsafe { self.ops.as_mut().strategy(bp, self.as_ptr()) }
    }

    pub fn bread(&mut self, block_number: u32) -> Arc<[u8]> {
        unsafe { self.ops.as_mut().bread(block_number, self.as_ptr()) }
    }
}

impl Drop for VNode {
    fn drop(&mut self) {
        let vnode_ops = unsafe { Box::from_raw(self.ops.as_ptr()) };
        drop(vnode_ops)
    }
}

impl Debug for VNode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("VNode"))
    }
}

#[repr(C)]
pub union NodeData {
    socket: (),      // Socket
    stream_data: (), // Stream
}

pub struct UserCred {
    pub uid: u16,
    pub gid: u16,
}

pub enum IODirection {
    Read,
    Write,
}

#[allow(unused)]
pub struct IoVec {
    iov_base: *mut u8,
    iov_len: usize,
}

#[allow(unused)]
pub struct UIO {
    iov: *mut IoVec,
    iov_count: u32,
    offset: usize,
    seg_flag: u32,
    file_mode: u32,
    max_offset: usize,
    residual_count: u32,
}

pub trait VNodeOperations {
    fn open(&mut self, f: u32, c: UserCred, vp: *const VNode) -> Result<Arc<[u8]>, ()>;
    fn close(&mut self, f: u32, c: UserCred, vp: *const VNode);
    fn rdwr(
        &mut self,
        uiop: *const UIO,
        direction: IODirection,
        f: u32,
        c: UserCred,
        vp: *const VNode,
    );
    fn ioctl(&mut self, com: u32, d: *mut u8, f: u32, c: UserCred, vp: *const VNode);
    fn select(&mut self, w: IODirection, c: UserCred, vp: *const VNode);
    fn getattr(&mut self, c: UserCred, vp: *const VNode) -> VAttr;
    fn setattr(&mut self, va: VAttr, c: UserCred, vp: *const VNode);
    fn access(&mut self, m: u32, c: UserCred, vp: *const VNode);
    fn lookup(&mut self, nm: &str, c: UserCred, vp: *const VNode) -> Result<VNode, ()>;
    fn create(
        &mut self,
        nm: &str,
        va: VAttr,
        e: u32,
        m: u32,
        c: UserCred,
        vp: *const VNode,
    ) -> Result<VNode, ()>;
    fn link(&mut self, target_dir: *mut VNode, target_name: &str, c: UserCred, vp: *const VNode);
    fn rename(
        &mut self,
        nm: &str,
        target_dir: *mut VNode,
        target_name: &str,
        c: UserCred,
        vp: *const VNode,
    );
    fn mkdir(&mut self, nm: &str, va: VAttr, c: UserCred, vp: *const VNode) -> Result<VNode, ()>;
    fn readdir(&mut self, uiop: *const UIO, c: UserCred, vp: *const VNode);
    fn symlink(
        &mut self,
        link_name: &str,
        va: VAttr,
        target_name: &str,
        c: UserCred,
        vp: *const VNode,
    );
    fn readlink(&mut self, uiop: *const UIO, c: UserCred, vp: *const VNode);
    fn fsync(&mut self, c: UserCred, vp: *const VNode);
    fn inactive(&mut self, c: UserCred, vp: *const VNode);
    fn bmap(&mut self, block_number: u32, bnp: (), vp: *const VNode) -> VNode;
    fn strategy(&mut self, bp: (), vp: *const VNode);
    fn bread(&mut self, block_number: u32, vp: *const VNode) -> Arc<[u8]>;
}

#[allow(unused)]
pub struct VAttr {
    typ: VNode,
    mode: u16,
    uid: u16,
    gid: u16,
    fs_id: u32,
    node_id: u32,
    link_count: u16,
    size: u32,
    block_size: u32,
    last_access: u32,
    last_modify: u32,
    // got no clue
    last_chg: u32,
    // the device???
    rdev: (),
    used_blocks: u32,
}

unsafe fn vfs_has_mount_point(mount_point: &str) -> Option<NonNull<Vfs>> {
    let mut current = ROOT_VFS.next;

    while let Some(node) = current {
        if node.as_ref().mount_point.as_ref().unwrap() == mount_point {
            return Some(node);
        }

        current = unsafe { (*node.as_ptr()).next };
    }

    None
}

pub fn add_vfs(mount_point: &str, fs_ops: Box<dyn FsOps>) -> Result<(), ()> {
    /// # Safety
    /// Consumes vfs
    unsafe fn deallocate_vfs(vfs: NonNull<Vfs>) {
        let fs_ops_box = Box::from_raw(vfs.as_ref().ops.unwrap().as_ptr());
        drop(fs_ops_box);
        dealloc(
            vfs.as_ptr().cast::<u8>(),
            alloc::alloc::Layout::new::<Vfs>(),
        );
    }

    let layout = alloc::alloc::Layout::new::<Vfs>();
    let vfs_ptr = unsafe { alloc(layout).cast::<Vfs>() };

    if vfs_ptr.is_null() {
        handle_alloc_error(layout)
    }

    // Initialize the data so we can use the nonnull helpers
    unsafe {
        let mut vfs = Vfs::null();
        vfs.ops = Some(NonNull::new_unchecked(Box::into_raw(fs_ops)));
        // 'normalize' the path (yes, making "/" == "" is intentional)
        vfs.mount_point = Some(mount_point.trim_end_matches('/').to_string());
        vfs_ptr.write(vfs);
    };

    // Safety: We know vfs_ptr is not null
    let mut vfs_ptr = unsafe { NonNull::new_unchecked(vfs_ptr) };

    let vfs = unsafe { vfs_ptr.as_mut() };

    log_info!("Adding vfs at {mount_point}");

    if mount_point == "/" {
        if unsafe { ROOT_VFS.next.is_some() } {
            unsafe {
                deallocate_vfs(vfs_ptr);
            };

            return Err(());
        }

        vfs.mount(mount_point);

        unsafe { ROOT_VFS.next = Some(vfs_ptr) };
    } else {
        if unsafe { ROOT_VFS.next.is_none() } {
            unsafe {
                deallocate_vfs(vfs_ptr);
            };
            return Err(());
        }

        if vfs_open(mount_point).is_err() {
            return Err(());
        }

        let mut next_vfs = unsafe { ROOT_VFS.next };

        while let Some(target_vfs) = next_vfs {
            if unsafe { target_vfs.as_ref().mount_point.as_ref().unwrap() == mount_point } {
                unsafe {
                    deallocate_vfs(vfs_ptr);
                };
                return Err(());
            }

            if unsafe { target_vfs.as_ref().next }.is_none() {
                break;
            }

            next_vfs = unsafe { target_vfs.as_ref().next };
        }

        if next_vfs.is_none() {
            unsafe {
                deallocate_vfs(vfs_ptr);
            };
            return Err(());
        }

        vfs.mount(mount_point);

        unsafe { (next_vfs.unwrap()).as_mut().next = Some(vfs_ptr) };
    }

    log_ok!("Added vfs at {mount_point}");

    return Ok(());
}

pub fn vfs_open(path: &str) -> Result<VNode, ()> {
    if unsafe { ROOT_VFS.next.is_none() } {
        return Err(());
    }

    let mut cur_vnode = unsafe { ROOT_VFS.next.unwrap().as_mut().root() };

    let parts = path.split('/').collect::<Vec<&str>>();

    for part in parts {
        if part.is_empty() {
            continue;
        }

        if let Ok(vnode) = cur_vnode.lookup(part, UserCred { uid: 0, gid: 0 }) {
            cur_vnode = vnode;
        } else {
            return Err(());
        }
    }

    return Ok(cur_vnode);
}
