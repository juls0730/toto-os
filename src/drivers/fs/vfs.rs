use core::{fmt::Debug, ptr::NonNull};

use alloc::{
    boxed::Box,
    collections::BTreeMap,
    rc::Rc,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};

use crate::{log_info, log_ok};

// TODO: probably keeps excess memory but whatever
struct TreeNode {
    vnode: Rc<VNode>,
    parent: Option<NonNull<Self>>,
    children: BTreeMap<String, Self>,
}

impl TreeNode {
    fn new(vnode: VNode) -> Self {
        return Self {
            vnode: Rc::new(vnode),
            parent: None,
            children: BTreeMap::new(),
        };
    }

    fn as_ptr(&self) -> NonNull<Self> {
        return NonNull::from(self);
    }

    fn get_vnode(&mut self) -> &mut VNode {
        Rc::get_mut(&mut self.vnode).unwrap()
    }

    fn lookup(&mut self, name: &str) -> Result<&mut Self, ()> {
        let parent = Some(self.as_ptr());

        crate::println!("looking up {name} in node_tree");

        if !self.children.contains_key(name) {
            crate::println!("not found in node tree");

            let vnode: VNode;

            if let Some(mut vfs) = self.vnode.vfs_mounted_here {
                crate::println!("using VFS root");

                unsafe {
                    vnode = vfs
                        .as_mut()
                        .root()
                        .lookup(name, UserCred { uid: 0, gid: 0 })?
                };
            } else {
                vnode = Rc::get_mut(&mut self.vnode)
                    .unwrap()
                    .lookup(name, UserCred { uid: 0, gid: 0 })?;
            }

            let child_node = TreeNode {
                vnode: Rc::new(vnode),
                parent,
                children: BTreeMap::new(),
            };

            self.children.insert(name.to_string(), child_node);
            let child = self.children.get_mut(name).unwrap();
            return Ok(child);
        }

        return Ok(self.children.get_mut(name).unwrap());
    }
}

static mut NODE_TREE: Option<TreeNode> = None;
static mut ROOT_VFS: Vfs = Vfs::null();

// TODO: everything being Option to accomodate the stupid null root vfs is getting annoying
#[allow(unused)]
pub struct Vfs {
    next: Option<Box<Vfs>>,
    pub fs: Option<Box<dyn FsOps>>,
    vnode_covered: Option<NonNull<VNode>>,
    flags: u32,
    block_size: u32,
    pub data: *mut u8,
}

impl !Sync for Vfs {}

impl Vfs {
    const fn null() -> Self {
        return Vfs {
            next: None,
            fs: None,
            vnode_covered: None,
            flags: 0,
            block_size: 0,
            data: core::ptr::null_mut(),
        };
    }

    fn add_vfs(&mut self, vfs: Box<Self>) {
        let mut current = self;
        while let Some(ref mut next_vfs) = current.next {
            current = next_vfs;
        }
        current.next = Some(vfs);
    }

    fn as_ptr(&self) -> NonNull<Self> {
        unsafe { NonNull::new_unchecked(core::ptr::addr_of!(*self) as *mut Self) }
    }

    pub fn mount(&mut self, path: &str) {
        if self.fs.is_none() {
            panic!("FsOps is null");
        }

        let vfsp = self.as_ptr();

        self.fs
            .as_mut()
            .unwrap()
            .as_mut()
            .mount(path, &mut self.data, vfsp);
    }

    pub fn unmount(&mut self) {
        if self.fs.is_none() {
            panic!("FsOps is null");
        }

        let vfsp = self.as_ptr();

        self.fs.as_mut().unwrap().as_mut().unmount(vfsp);
    }

    pub fn root(&mut self) -> VNode {
        if self.fs.is_none() {
            panic!("FsOps is null");
        }

        let vfsp = self.as_ptr();

        self.fs.as_mut().unwrap().as_mut().root(vfsp)
    }

    pub fn statfs(&mut self) -> StatFs {
        if self.fs.is_none() {
            panic!("FsOps is null");
        }

        let vfsp = self.as_ptr();

        self.fs.as_mut().unwrap().as_mut().statfs(vfsp)
    }

    pub fn sync(&mut self) {
        if self.fs.is_none() {
            panic!("FsOps is null");
        }

        let vfsp = self.as_ptr();

        self.fs.as_mut().unwrap().as_mut().sync(vfsp);
    }

    pub fn fid(&mut self, path: &str) -> Option<FileId> {
        if self.fs.is_none() {
            panic!("FsOps is null");
        }

        let vfsp = self.as_ptr();

        self.fs.as_mut().unwrap().as_mut().fid(path, vfsp)
    }

    pub fn vget(&mut self, fid: FileId) -> VNode {
        if self.fs.is_none() {
            panic!("FsOps is null");
        }

        let vfsp = self.as_ptr();

        self.fs.as_mut().unwrap().as_mut().vget(fid, vfsp)
    }
}

pub trait FsOps {
    // yes, the vfsp was the best solution I could come up with
    fn mount(&mut self, path: &str, data: &mut *mut u8, vfsp: NonNull<Vfs>);
    fn unmount(&mut self, vfsp: NonNull<Vfs>);
    fn root(&mut self, vfsp: NonNull<Vfs>) -> VNode;
    fn statfs(&mut self, vfsp: NonNull<Vfs>) -> StatFs;
    fn sync(&mut self, vfsp: NonNull<Vfs>);
    fn fid(&mut self, path: &str, vfsp: NonNull<Vfs>) -> Option<FileId>;
    // idk how the fuck you're supposed to accomplish this
    // good luck I guess.
    fn vget(&mut self, fid: FileId, vfsp: NonNull<Vfs>) -> VNode;
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
#[derive(PartialEq)]
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
    inode: Box<dyn VNodeOperations>,
    pub node_data: Option<NodeData>,
    pub vfs_mounted_here: Option<NonNull<Vfs>>,
    pub parent_vfs: NonNull<Vfs>,
    pub file_typ: VNodeType,
    pub data: *mut u8,
}

impl VNode {
    pub fn new(
        inode: Box<dyn VNodeOperations>,
        file_typ: VNodeType,
        parent_vfs: NonNull<Vfs>,
    ) -> Self {
        return Self {
            flags: 0,
            inode,
            node_data: None,
            vfs_mounted_here: None,
            parent_vfs,
            file_typ,
            data: core::ptr::null_mut(),
        };
    }

    pub fn as_ptr(&self) -> NonNull<VNode> {
        unsafe { NonNull::new_unchecked(core::ptr::addr_of!(*self) as *mut Self) }
    }

    // Trait functions
    pub fn open(&mut self, f: u32, c: UserCred) -> Result<Arc<[u8]>, ()> {
        let vp = self.as_ptr();

        self.inode.as_mut().open(f, c, vp)
    }

    pub fn close(&mut self, f: u32, c: UserCred) {
        let vp = self.as_ptr();

        self.inode.as_mut().close(f, c, vp)
    }

    pub fn rdwr(
        &mut self,
        uiop: *const UIO,
        direction: IODirection,
        f: u32,
        c: UserCred,
    ) -> Result<Arc<[u8]>, ()> {
        let vp = self.as_ptr();

        self.inode.as_mut().rdwr(uiop, direction, f, c, vp)
    }

    pub fn ioctl(&mut self, com: u32, d: *mut u8, f: u32, c: UserCred) {
        let vp = self.as_ptr();

        self.inode.as_mut().ioctl(com, d, f, c, vp)
    }

    pub fn select(&mut self, w: IODirection, c: UserCred) {
        let vp = self.as_ptr();

        self.inode.as_mut().select(w, c, vp)
    }

    pub fn getattr(&mut self, c: UserCred) -> VAttr {
        let vp = self.as_ptr();

        self.inode.as_mut().getattr(c, vp)
    }

    pub fn setattr(&mut self, va: VAttr, c: UserCred) {
        let vp = self.as_ptr();

        self.inode.as_mut().setattr(va, c, vp)
    }

    pub fn access(&mut self, m: u32, c: UserCred) {
        let vp = self.as_ptr();

        self.inode.as_mut().access(m, c, vp)
    }

    pub fn lookup(&mut self, nm: &str, c: UserCred) -> Result<VNode, ()> {
        let vp = self.as_ptr();

        self.inode.as_mut().lookup(nm, c, vp)
    }

    pub fn create(
        &mut self,
        nm: &str,
        va: VAttr,
        e: u32,
        m: u32,
        c: UserCred,
    ) -> Result<VNode, ()> {
        let vp = self.as_ptr();

        self.inode.as_mut().create(nm, va, e, m, c, vp)
    }

    pub fn link(&mut self, target_dir: *mut VNode, target_name: &str, c: UserCred) {
        let vp = self.as_ptr();

        self.inode.as_mut().link(target_dir, target_name, c, vp)
    }

    pub fn rename(&mut self, nm: &str, target_dir: *mut VNode, target_name: &str, c: UserCred) {
        let vp = self.as_ptr();

        self.inode
            .as_mut()
            .rename(nm, target_dir, target_name, c, vp)
    }

    pub fn mkdir(&mut self, nm: &str, va: VAttr, c: UserCred) -> Result<VNode, ()> {
        let vp = self.as_ptr();

        self.inode.as_mut().mkdir(nm, va, c, vp)
    }

    pub fn readdir(&mut self, uiop: *const UIO, c: UserCred) {
        let vp = self.as_ptr();

        self.inode.as_mut().readdir(uiop, c, vp)
    }

    pub fn symlink(&mut self, link_name: &str, va: VAttr, target_name: &str, c: UserCred) {
        let vp = self.as_ptr();

        self.inode
            .as_mut()
            .symlink(link_name, va, target_name, c, vp)
    }

    pub fn readlink(&mut self, uiop: *const UIO, c: UserCred) {
        let vp = self.as_ptr();

        self.inode.as_mut().readlink(uiop, c, vp)
    }

    pub fn fsync(&mut self, c: UserCred) {
        let vp = self.as_ptr();

        self.inode.as_mut().fsync(c, vp)
    }

    pub fn inactive(&mut self, c: UserCred) {
        let vp = self.as_ptr();

        self.inode.as_mut().inactive(c, vp)
    }

    pub fn bmap(&mut self, block_number: u32, bnp: ()) -> VNode {
        let vp = self.as_ptr();

        self.inode.as_mut().bmap(block_number, bnp, vp)
    }

    pub fn strategy(&mut self, bp: ()) {
        let vp = self.as_ptr();

        self.inode.as_mut().strategy(bp, vp)
    }

    pub fn bread(&mut self, block_number: u32) -> Arc<[u8]> {
        let vp = self.as_ptr();

        self.inode.as_mut().bread(block_number, vp)
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
    fn open(&mut self, f: u32, c: UserCred, vp: NonNull<VNode>) -> Result<Arc<[u8]>, ()>;
    fn close(&mut self, f: u32, c: UserCred, vp: NonNull<VNode>);
    fn rdwr(
        &mut self,
        uiop: *const UIO,
        direction: IODirection,
        f: u32,
        c: UserCred,
        vp: NonNull<VNode>,
    ) -> Result<Arc<[u8]>, ()>;
    fn ioctl(&mut self, com: u32, d: *mut u8, f: u32, c: UserCred, vp: NonNull<VNode>);
    fn select(&mut self, w: IODirection, c: UserCred, vp: NonNull<VNode>);
    fn getattr(&mut self, c: UserCred, vp: NonNull<VNode>) -> VAttr;
    fn setattr(&mut self, va: VAttr, c: UserCred, vp: NonNull<VNode>);
    fn access(&mut self, m: u32, c: UserCred, vp: NonNull<VNode>);
    fn lookup(&mut self, nm: &str, c: UserCred, vp: NonNull<VNode>) -> Result<VNode, ()>;
    fn create(
        &mut self,
        nm: &str,
        va: VAttr,
        e: u32,
        m: u32,
        c: UserCred,
        vp: NonNull<VNode>,
    ) -> Result<VNode, ()>;
    fn link(&mut self, target_dir: *mut VNode, target_name: &str, c: UserCred, vp: NonNull<VNode>);
    fn rename(
        &mut self,
        nm: &str,
        target_dir: *mut VNode,
        target_name: &str,
        c: UserCred,
        vp: NonNull<VNode>,
    );
    fn mkdir(&mut self, nm: &str, va: VAttr, c: UserCred, vp: NonNull<VNode>) -> Result<VNode, ()>;
    fn readdir(&mut self, uiop: *const UIO, c: UserCred, vp: NonNull<VNode>);
    fn symlink(
        &mut self,
        link_name: &str,
        va: VAttr,
        target_name: &str,
        c: UserCred,
        vp: NonNull<VNode>,
    );
    fn readlink(&mut self, uiop: *const UIO, c: UserCred, vp: NonNull<VNode>);
    fn fsync(&mut self, c: UserCred, vp: NonNull<VNode>);
    fn inactive(&mut self, c: UserCred, vp: NonNull<VNode>);
    fn bmap(&mut self, block_number: u32, bnp: (), vp: NonNull<VNode>) -> VNode;
    fn strategy(&mut self, bp: (), vp: NonNull<VNode>);
    fn bread(&mut self, block_number: u32, vp: NonNull<VNode>) -> Arc<[u8]>;
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

pub fn add_vfs(mount_point: &str, fs_ops: Box<dyn FsOps>) -> Result<(), ()> {
    // Initialize the data so we can use the nonnull helpers
    let mut new_vfs = Vfs::null();
    new_vfs.fs = Some(fs_ops);
    let mut vfs = Box::new(new_vfs);

    let vfsp = vfs.as_ptr();

    log_info!("Adding vfs at {mount_point}");

    if mount_point == "/" {
        if unsafe { ROOT_VFS.next.is_some() } {
            return Err(());
        }

        unsafe { NODE_TREE = Some(TreeNode::new(vfs.fs.as_mut().unwrap().as_mut().root(vfsp))) }
    } else {
        if unsafe { ROOT_VFS.next.is_none() } {
            return Err(());
        }

        if vfs_open(mount_point).is_err() {
            return Err(());
        }

        let vnode = vfs_open(mount_point)?;

        vnode.vfs_mounted_here = Some(vfsp);
    }

    vfs.mount(mount_point);

    unsafe { ROOT_VFS.add_vfs(vfs) };

    log_ok!("Added vfs at {mount_point}");

    return Ok(());
}

pub fn vfs_open(path: &str) -> Result<&mut VNode, ()> {
    if unsafe { ROOT_VFS.next.is_none() || NODE_TREE.is_none() } {
        return Err(());
    }

    let mut cur_node = unsafe { NODE_TREE.as_mut().unwrap() };

    let parts = path.split('/').collect::<Vec<&str>>();

    for part in parts {
        if part.is_empty() {
            continue;
        }

        if let Ok(new_node) = cur_node.lookup(part) {
            cur_node = new_node;
        } else {
            return Err(());
        }
    }

    return Ok(cur_node.get_vnode());
}
