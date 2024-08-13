use core::{fmt::Debug, ptr::NonNull};

use alloc::{
    boxed::Box,
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};

use crate::{log_info, log_ok};

static mut NODE_TREE: Option<TreeNode> = None;
static mut ROOT_VFS: Vfs = Vfs::null();

// TODO: everything being Option to accomodate the stupid null root vfs is getting annoying
#[allow(unused)]
pub struct Vfs {
    mount_point: Option<String>,
    next: Option<Box<Self>>,
    prev: Option<NonNull<Self>>,
    pub fs: Option<Box<dyn FsOps>>,
    vnode_covered: Option<NonNull<VNode>>,
    flags: u32,
    block_size: u32,
    pub data: *mut u8,
}

impl !Sync for Vfs {}

impl Vfs {
    const fn null() -> Self {
        return Self {
            mount_point: None,
            next: None,
            prev: None,
            fs: None,
            vnode_covered: None,
            flags: 0,
            block_size: 0,
            data: core::ptr::null_mut(),
        };
    }

    fn new(fs: Box<dyn FsOps>, mount_point: &str) -> Self {
        return Self {
            mount_point: Some(mount_point.to_string()),
            next: None,
            prev: None,
            fs: Some(fs),
            vnode_covered: None,
            flags: 0,
            block_size: 0,
            data: core::ptr::null_mut(),
        };
    }

    fn del_vfs(&mut self, target_name: &str) {
        let mut curr = self.next.as_mut();

        while let Some(node) = curr {
            if node.mount_point.as_deref() == Some(target_name) {
                if let Some(ref mut next_node) = node.next {
                    next_node.prev = node.prev
                }
                unsafe { node.prev.unwrap().as_mut().next = node.next.take() };
                return;
            }

            curr = node.next.as_mut();
        }
    }

    fn add_vfs(&mut self, mut vfs: Box<Self>) {
        let mut current = self;
        while let Some(ref mut next_vfs) = current.next {
            current = next_vfs;
        }

        vfs.prev = Some(current.as_ptr());
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

pub struct File {
    descriptor: NonNull<TreeNode>,
    user_cred: UserCred,
}

impl File {
    fn new(tree_node: NonNull<TreeNode>, user_cred: UserCred) -> Self {
        return Self {
            descriptor: tree_node,
            user_cred,
        };
    }

    fn get_node(&mut self) -> &mut TreeNode {
        unsafe { self.descriptor.as_mut() }
    }

    pub fn read(&mut self, mut count: usize, offset: usize, f: u32) -> Result<Arc<[u8]>, ()> {
        if count == 0 {
            count = self.len() - offset;
        }

        return self.get_node().read(count, offset, f);
    }

    pub fn write(&mut self, offset: usize, buf: &[u8], f: u32) {
        self.get_node().write(offset, buf, f);
    }

    pub fn len(&mut self) -> usize {
        self.get_node().len()
    }
}

impl Drop for File {
    fn drop(&mut self) {
        let cred = self.user_cred;

        self.get_node().close(0, cred);
    }
}

// TODO: probably keeps excess memory but whatever
pub struct TreeNode {
    vnode: VNode,
    parent: Option<NonNull<Self>>,
    children: BTreeMap<String, Self>,
}

impl TreeNode {
    fn new(vnode: VNode) -> Self {
        return Self {
            vnode: vnode,
            parent: None,
            children: BTreeMap::new(),
        };
    }

    fn as_ptr(&self) -> NonNull<Self> {
        return NonNull::from(self);
    }

    fn get_vnode_mut(&mut self) -> &mut VNode {
        &mut self.vnode
    }

    fn get_vnode(&self) -> &VNode {
        &self.vnode
    }

    pub fn lookup(&mut self, name: &str) -> Result<&mut Self, ()> {
        let parent = Some(self.as_ptr());

        if !self.children.contains_key(name) {
            let vnode: VNode;

            if let Some(mut vfs) = self.vnode.vfs_mounted_here {
                unsafe {
                    vnode = vfs
                        .as_mut()
                        .root()
                        .lookup(name, UserCred { uid: 0, gid: 0 })?
                };
            } else {
                vnode = self
                    .get_vnode_mut()
                    .lookup(name, UserCred { uid: 0, gid: 0 })?;
            }

            let child_node = TreeNode {
                vnode: vnode,
                parent,
                children: BTreeMap::new(),
            };

            self.children.insert(name.to_string(), child_node);
            let child = self.children.get_mut(name).unwrap();
            return Ok(child);
        }

        return Ok(self.children.get_mut(name).unwrap());
    }

    fn read(&mut self, count: usize, offset: usize, f: u32) -> Result<Arc<[u8]>, ()> {
        self.get_vnode_mut()
            .read(count, offset, f, UserCred { uid: 0, gid: 0 })
    }

    fn write(&mut self, offset: usize, buf: &[u8], f: u32) {
        self.get_vnode_mut()
            .write(offset, buf, f, UserCred { uid: 0, gid: 0 })
    }

    pub fn open(&mut self, f: u32, c: UserCred) -> File {
        let vnode = self.get_vnode_mut();

        if vnode.ref_count == 0 {
            vnode.open(f, c);
        }

        vnode.ref_count += 1;

        return File::new(self.as_ptr(), c);
    }

    fn close(&mut self, f: u32, c: UserCred) {
        let vnode = self.get_vnode_mut();

        vnode.ref_count -= 1;

        if vnode.ref_count == 0 {
            vnode.close(f, c)
        }
    }

    fn len(&self) -> usize {
        self.get_vnode().len()
    }
}

pub struct VNode {
    pub flags: u16,
    ref_count: u32,
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
            ref_count: 0,
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
    pub fn open(&mut self, f: u32, c: UserCred) {
        let vp = self.as_ptr();

        self.inode.as_mut().open(f, c, vp)
    }

    pub fn close(&mut self, f: u32, c: UserCred) {
        let vp = self.as_ptr();

        self.inode.as_mut().close(f, c, vp)
    }

    pub fn read(
        &mut self,
        count: usize,
        offset: usize,
        f: u32,
        c: UserCred,
    ) -> Result<Arc<[u8]>, ()> {
        if offset >= self.len() || count > self.len() || count + offset > self.len() {
            return Err(());
        }

        let vp = self.as_ptr();

        self.inode.as_mut().read(count, offset, f, c, vp)
    }

    pub fn write(&mut self, offset: usize, buf: &[u8], f: u32, c: UserCred) {
        let vp = self.as_ptr();

        self.inode.as_mut().write(offset, buf, f, c, vp)
    }
    pub fn ioctl(&mut self, com: u32, d: *mut u8, f: u32, c: UserCred) {
        let vp = self.as_ptr();

        self.inode.as_mut().ioctl(com, d, f, c, vp)
    }

    // pub fn select(&mut self, w: IODirection, c: UserCred) {
    //     let vp = self.as_ptr();

    //     self.inode.as_mut().select(w, c, vp)
    // }

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

    // pub fn inactive(&mut self, c: UserCred) {
    //     let vp = self.as_ptr();

    //     self.inode.as_mut().inactive(c, vp)
    // }

    // pub fn bmap(&mut self, block_number: u32, bnp: ()) -> VNode {
    //     let vp = self.as_ptr();

    //     self.inode.as_mut().bmap(block_number, bnp, vp)
    // }

    // pub fn strategy(&mut self, bp: ()) {
    //     let vp = self.as_ptr();

    //     self.inode.as_mut().strategy(bp, vp)
    // }

    // pub fn bread(&mut self, block_number: u32) -> Arc<[u8]> {
    //     let vp = self.as_ptr();

    //     self.inode.as_mut().bread(block_number, vp)
    // }

    pub fn len(&self) -> usize {
        let vp = self.as_ptr();

        self.inode.as_ref().len(vp)
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

#[derive(Clone, Copy)]
pub struct UserCred {
    pub uid: u16,
    pub gid: u16,
}

// pub enum IODirection {
//     Read,
//     Write,
// }

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
    fn open(&mut self, f: u32, c: UserCred, vp: NonNull<VNode>);
    fn close(&mut self, f: u32, c: UserCred, vp: NonNull<VNode>);
    fn read(
        &mut self,
        count: usize,
        offset: usize,
        f: u32,
        c: UserCred,
        vp: NonNull<VNode>,
    ) -> Result<Arc<[u8]>, ()>;
    fn write(&mut self, offset: usize, buf: &[u8], f: u32, c: UserCred, vp: NonNull<VNode>);
    fn ioctl(&mut self, com: u32, d: *mut u8, f: u32, c: UserCred, vp: NonNull<VNode>);
    // fn select(&mut self, w: IODirection, c: UserCred, vp: NonNull<VNode>);
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
    // fn inactive(&mut self, c: UserCred, vp: NonNull<VNode>);
    // fn bmap(&mut self, block_number: u32, bnp: (), vp: NonNull<VNode>) -> VNode;
    // fn strategy(&mut self, bp: (), vp: NonNull<VNode>);
    // fn bread(&mut self, block_number: u32, vp: NonNull<VNode>) -> Arc<[u8]>;
    fn len(&self, vp: NonNull<VNode>) -> usize;

    // TODO: not object safe
    // fn get_fs<'a, T>(&self, vp: NonNull<VNode>) -> &'a mut T
    // where
    //     T: FsOps,
    // {
    //     unsafe {
    //         let vfs = (*vp.as_ptr()).parent_vfs.as_mut();
    //         let fs = vfs
    //             .fs
    //             .as_mut()
    //             .expect("Tried to call get_fs on root VFS")
    //             .as_mut() as *mut dyn FsOps;
    //         &mut *fs.cast::<T>()
    //     }
    // }
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
    let mut vfs = Box::new(Vfs::new(fs_ops, mount_point));

    let vfsp = vfs.as_ptr();

    log_info!("Adding vfs at {mount_point}");

    if mount_point == "/" {
        if unsafe { ROOT_VFS.next.is_some() } {
            return Err(());
        }

        crate::println!("reading the root");
        let root = vfs.fs.as_mut().unwrap().as_mut().root(vfsp);
        crate::println!("successfully read the root");
        unsafe { NODE_TREE = Some(TreeNode::new(root)) }
    } else {
        if unsafe { ROOT_VFS.next.is_none() } {
            return Err(());
        }

        if vfs_open(mount_point).is_err() {
            return Err(());
        }

        let file = vfs_open(mount_point)?;

        file.get_vnode_mut().vfs_mounted_here = Some(vfsp);
    }

    vfs.mount(mount_point);

    unsafe { ROOT_VFS.add_vfs(vfs) };

    log_ok!("Added vfs at {mount_point}");

    return Ok(());
}

// returns if the path in other_mount_point starts with mount_point but more sophisticated-ly
fn mount_point_busy(mount_point: &str) -> bool {
    let mount_parts = mount_point
        .split_terminator('/')
        .filter(|x| !x.is_empty())
        .collect::<Vec<&str>>();
    let mut next_vfs = unsafe { ROOT_VFS.next.as_ref() };

    while let Some(vfs) = next_vfs {
        if vfs.mount_point.as_ref().unwrap() == mount_point {
            // dont consider ourself as a user of ourself
            next_vfs = vfs.next.as_ref();
            continue;
        }

        let parts = vfs
            .mount_point
            .as_ref()
            .unwrap()
            .split_terminator('/')
            .filter(|x| !x.is_empty())
            .collect::<Vec<&str>>();

        for (i, &part) in parts.iter().enumerate() {
            if i > mount_parts.len() - 1 {
                return true;
            }

            if part == mount_parts[i] {
                continue;
            } else {
                break;
            }
        }

        next_vfs = vfs.next.as_ref();
    }

    return false;
}

pub fn del_vfs(mount_point: &str) -> Result<(), ()> {
    if unsafe { ROOT_VFS.next.is_none() } {
        return Err(());
    }

    log_info!("Deleting vfs at {mount_point}");

    if mount_point == "/" {
        if unsafe { ROOT_VFS.next.as_ref().unwrap().next.is_some() } {
            // mount point is 'busy'
            return Err(());
        }

        unsafe { ROOT_VFS.next = None };
    } else {
        if mount_point_busy(mount_point) {
            return Err(());
        }

        unsafe { ROOT_VFS.del_vfs(mount_point) };
    }

    return Ok(());
}

pub fn vfs_open(path: &str) -> Result<&mut TreeNode, ()> {
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

    return Ok(cur_node);
}
