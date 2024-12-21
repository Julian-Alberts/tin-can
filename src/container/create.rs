use core::panic;
use std::{fmt::Debug, path::PathBuf, pin::Pin, ptr::addr_of_mut, sync::Mutex};

use crate::linux::{self, switch_user};

use super::*;

#[derive(Debug, thiserror::Error)]
pub enum CreateError {
    #[error("Unshare: {0}")]
    Unshare(#[from] linux::UnshareError),
    #[error("Switch User: {0}")]
    SwitchUser(#[from] linux::SwitchUserError),
}

pub struct ProcessHandle<F: FnOnce() -> i32 + Send + 'static> {
    intern: linux::ProcessHandle<'static>,
    process_data: Pin<Box<ProcessData<F>>>,
}

struct ProcessData<F: FnOnce() -> i32 + Send + 'static> {
    ctp_tx: std::sync::mpsc::Sender<MsgChildToParent>,
    ptc_rx: std::sync::mpsc::Receiver<MsgParentToChild>,
    container: Container<Validated>,
    f: Mutex<Option<F>>,
}

impl<F> ProcessHandle<F>
where
    F: FnOnce() -> i32 + Send + 'static,
{
    pub fn pid(&self) -> libc::pid_t {
        self.intern.pid
    }

    pub fn join(self) -> i32 {
        eprintln!("outer join");
        self.intern.join()
    }
}

impl<F> std::fmt::Debug for ProcessData<F>
where
    F: FnOnce() -> i32 + Send + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessData")
            .field("ctp_tx", &self.ctp_tx)
            .field("ptc_rx", &self.ptc_rx)
            .field("container", &self.container)
            .finish()
    }
}

impl Container<Validated> {
    pub fn create<'a, F: FnOnce() -> i32 + Send + 'static>(self, f: F) -> ProcessHandle<F> {
        let (ctp_tx, ctp_rx) = std::sync::mpsc::channel();
        let (ptc_tx, ptc_rx) = std::sync::mpsc::channel();

        let mut process_data = Box::pin(ProcessData {
            ctp_tx,
            ptc_rx,
            container: self,
            f: Mutex::new(Some(f)),
        });
        let process_handle_ref =
            unsafe { (&mut process_data as *mut Pin<Box<ProcessData<F>>>).as_mut() }.unwrap();
        let pid = linux::clone_vm_with_namespaces(
            libc::CLONE_NEWUSER,
            container_main,
            process_handle_ref,
        )
        .unwrap();
        if pid.pid == -1 {
            panic!("{pid:?}")
        }

        let handle = ProcessHandle {
            intern: pid,
            process_data,
        };

        while let Ok(msg) = ctp_rx.recv() {
            match msg {
                MsgChildToParent::MapUserIds => {
                    create_user_mapping(
                        handle.process_data.container.user.as_ref().unwrap(),
                        handle.pid(),
                    );
                    ptc_tx.send(MsgParentToChild::UserIdsMapped).unwrap()
                }
                MsgChildToParent::Success => break,
            }
        }

        eprintln!("created2");
        handle
    }
}

fn container_main<F: FnOnce() -> i32 + Send + 'static>(
    handle: &mut Pin<Box<ProcessData<F>>>,
) -> i32 {
    print!("TTTTEST");
    let mut is_root = false;
    if let Some(user) = &handle.container.user {
        let _ = handle.ctp_tx.send(MsgChildToParent::MapUserIds);
        let Ok(MsgParentToChild::UserIdsMapped) = handle.ptc_rx.recv() else {
            panic!("Unexpected message")
        };
        if user.uid_map.iter().any(|m| m.start_intern == 0)
            && user.gid_map.iter().any(|m| m.start_intern == 0)
        {
            switch_user((0, 0));
            is_root = true;
        }
    }
    eprintln!("created");
    handle.ctp_tx.send(MsgChildToParent::Success).unwrap();
    std::thread::sleep(std::time::Duration::new(1, 0));
    (handle.f.lock().unwrap().take().unwrap())()
}

//fn mount(mount: MountNamespace) {}

fn create_user_mapping(user: &UserNamespace<Validated>, tid: libc::pid_t) {
    let mut path = PathBuf::from("/proc/");
    path.push(tid.to_string());
    let uid = path.join("uid_map");
    let gid = path.join("gid_map");
    let mut uid = std::fs::File::create(uid).unwrap();
    let mut gid = std::fs::File::create(gid).unwrap();
    user.uid_map.iter().try_for_each(|entry| {
        use std::io::Write;
        writeln!(
            uid,
            "{} {} {}",
            entry.start_intern, entry.start_extern, entry.len
        )
    });
    user.gid_map.iter().try_for_each(|entry| {
        use std::io::Write;
        writeln!(
            gid,
            "{} {} {}",
            entry.start_intern, entry.start_extern, entry.len
        )
    });
}

enum MsgChildToParent {
    MapUserIds,
    Success,
}
enum MsgParentToChild {
    UserIdsMapped,
}
