use core::fmt::Debug;

use alloc::{
    // alloc::{alloc, dealloc},
    boxed::Box,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};

use crate::{log_info, log_ok, mem::PHYSICAL_MEMORY_MANAGER};

static mut ROOT_VFS: Vfs = Vfs::null();

#[allow(unused)]
pub struct Vfs {
    mount_point: Option<String>,
    next: Option<*mut Vfs>,
    ops: Option<Box<dyn FsOps>>,
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

    fn as_ptr(&self) -> *const Vfs {
        core::ptr::addr_of!(*self)
    }

    fn as_mut_ptr(&mut self) -> *mut Vfs {
        core::ptr::addr_of_mut!(*self)
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
    pub flags: u16,
    pub ref_count: u16,
    pub shared_lock_count: u16,
    pub exclusive_lock_count: u16,
    pub ops: Box<dyn VNodeOperations>,
    pub node_data: Option<NodeData>,
    pub parent: *const Vfs,
    pub typ: VNodeType,
    pub data: *mut u8,
}

impl VNode {
    pub fn as_ptr(&self) -> *const VNode {
        core::ptr::addr_of!(*self)
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

unsafe fn find_mount_point(file_path: &str) -> Option<*mut Vfs> {
    // TODO: refactor
    let mut mount_point = ROOT_VFS.next;
    let mut current = ROOT_VFS.next;

    while let Some(node) = current {
        let mount_point_str = node
            .as_ref()
            .unwrap()
            .mount_point
            .as_ref()
            .expect("Null mount point");
        if file_path.starts_with(mount_point_str)
            && mount_point_str.len()
                > (mount_point.unwrap().as_ref().unwrap())
                    .mount_point
                    .as_ref()
                    .unwrap()
                    .len()
        {
            mount_point = Some(node);
        }
        current = unsafe { (*node).next };
    }

    mount_point
}

pub fn add_vfs(mut mount_point: &str, fs_ops: Box<dyn FsOps>) -> Result<(), ()> {
    if mount_point != "/" {
        mount_point = mount_point.trim_end_matches('/');
    }

    // let layout = alloc::alloc::Layout::new::<Vfs>();
    // TODO: its fucking broken again
    let vfs_ptr = PHYSICAL_MEMORY_MANAGER.alloc(1).cast::<Vfs>();

    let vfs = unsafe { &mut *vfs_ptr };

    (*vfs) = Vfs::null();
    (*vfs).ops = Some(fs_ops);
    (*vfs).mount_point = Some(mount_point.to_string());

    log_info!("Adding vfs at {mount_point} {vfs_ptr:p}");

    // TODO: dont give / special treatment
    if mount_point == "/" {
        if unsafe { ROOT_VFS.next.is_some() } {
            // unsafe { dealloc(vfs_ptr.cast::<u8>(), layout) };
            PHYSICAL_MEMORY_MANAGER.dealloc(vfs_ptr.cast::<u8>(), 1);
            return Err(());
        }

        {
            let vfsp = vfs.as_ptr();

            (*vfs)
                .ops
                .as_mut()
                .unwrap()
                .mount(mount_point, &mut vfs.data, vfsp);
        }

        unsafe { ROOT_VFS.next = Some(vfs.as_mut_ptr()) };
    } else {
        // TODO: technically this allows you to mount file systems at nonexistent mount point
        if unsafe { ROOT_VFS.next.is_none() } {
            // unsafe { dealloc(vfs_ptr.cast::<u8>(), layout) };
            PHYSICAL_MEMORY_MANAGER.dealloc(vfs_ptr.cast::<u8>(), 1);
            return Err(());
        }

        // let target_vfs = unsafe { ROOT_VFS.next.unwrap() };

        let mut next_vfs = unsafe { ROOT_VFS.next };

        while let Some(target_vfs) = next_vfs {
            if unsafe { target_vfs.as_ref().unwrap().mount_point.as_ref().unwrap() == mount_point }
            {
                // unsafe { dealloc(vfs_ptr.cast::<u8>(), layout) };
                PHYSICAL_MEMORY_MANAGER.dealloc(vfs_ptr.cast::<u8>(), 1);
                return Err(());
            }

            if unsafe { (*target_vfs).next }.is_none() {
                break;
            }

            next_vfs = unsafe { (*target_vfs).next };
        }

        if next_vfs.is_none() {
            // unsafe { dealloc(vfs_ptr.cast::<u8>(), layout) };
            PHYSICAL_MEMORY_MANAGER.dealloc(vfs_ptr.cast::<u8>(), 1);
            return Err(());
        }

        {
            let vfsp = vfs.as_ptr();

            (*vfs)
                .ops
                .as_mut()
                .unwrap()
                .mount(mount_point, &mut vfs.data, vfsp);
        }

        unsafe { (*(next_vfs.unwrap())).next = Some(vfs) };

        // let mut cur_vnode = unsafe { (*target_vfs).ops.as_mut().unwrap().root(target_vfs) };

        // let parts = mount_point.split('/').collect::<Vec<&str>>();

        // for part in parts {
        //     if part.is_empty() {
        //         continue;
        //     }

        //     // TODO: dont just lookup everything as the root user
        //     if let Ok(vnode) =
        //         cur_vnode
        //             .ops
        //             .lookup(part, UserCred { uid: 0, gid: 0 }, cur_vnode.as_ptr())
        //     {
        //         cur_vnode = vnode;
        //     } else {
        //         unsafe { dealloc(vfs_ptr.cast::<u8>(), layout) };
        //         return Err(());
        //     }
        // }

        // if cur_vnode.vfs_mounted_here.is_some() {
        //     unsafe { dealloc(vfs_ptr.cast::<u8>(), layout) };
        //     return Err(());
        // }

        // {
        //     let vfsp = vfs.as_ptr();

        // }

        // cur_vnode.vfs_mounted_here = Some(vfs.as_mut_ptr());
    }

    log_ok!("Added vfs at {mount_point}");

    return Ok(());
}

pub fn vfs_open(path: &str) -> Result<VNode, ()> {
    if unsafe { ROOT_VFS.next.is_none() } {
        return Err(());
    }

    let root_vfs = unsafe { find_mount_point(path) };

    if root_vfs.is_none() {
        return Err(());
    }

    let mut cur_vnode = unsafe {
        (*root_vfs.unwrap())
            .ops
            .as_mut()
            .unwrap()
            .root(root_vfs.unwrap())
    };

    let path = &path[unsafe { (*root_vfs.unwrap()).mount_point.as_ref().unwrap() }.len()..];

    let parts = path.split('/').collect::<Vec<&str>>();

    for part in parts {
        if part.is_empty() {
            continue;
        }

        if let Ok(vnode) =
            cur_vnode
                .ops
                .lookup(part, UserCred { uid: 0, gid: 0 }, cur_vnode.as_ptr())
        {
            cur_vnode = vnode;
        } else {
            return Err(());
        }
    }

    return Ok(cur_vnode);
}
