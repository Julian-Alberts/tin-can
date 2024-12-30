use core::panic;
use std::{path::PathBuf, pin::Pin};

use crate::linux::{self, switch_user};

use super::*;

#[derive(Debug, thiserror::Error)]
pub enum CreateError {
    #[error("Unshare: {0}")]
    Unshare(#[from] linux::UnshareError),
    #[error("Switch User: {0}")]
    SwitchUser(#[from] linux::SwitchUserError),
}

pub struct ProcessHandle<T> {
    intern: linux::ProcessHandle<'static>,
    process_data: ProcessData<T>,
}

#[derive(Debug)]
struct ProcessData<T> {
    ctp_tx: std::sync::mpsc::Sender<MsgChildToParent>,
    ptc_rx: std::sync::mpsc::Receiver<MsgParentToChild>,
    container: Container<Validated>,
    container_main: fn(&mut T) -> i32,
    main_args: T,
}

impl<T> ProcessHandle<T> {
    pub fn pid(&self) -> libc::pid_t {
        self.intern.pid
    }

    pub fn join(self) -> (i32, T) {
        eprintln!("outer join");
        (self.intern.join(), self.process_data.main_args)
    }
}

impl Container<Validated> {
    pub fn create<T: 'static>(
        self,
        container_main: fn(&mut T) -> i32,
        main_args: T,
    ) -> ProcessHandle<T> {
        let (ctp_tx, ctp_rx) = std::sync::mpsc::channel();
        let (ptc_tx, ptc_rx) = std::sync::mpsc::channel();

        let mut process_data = ProcessData {
            ctp_tx,
            ptc_rx,
            container: self,
            container_main,
            main_args,
        };
        let process_handle_ref =
            unsafe { (&mut process_data as *mut ProcessData<T>).as_mut() }.unwrap();
        let pid = linux::clone_vm_with_namespaces(
            libc::CLONE_NEWUSER,
            namespace_main,
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
            eprintln!("MSG {msg:?}");
            match msg {
                MsgChildToParent::MapUserIds => {
                    create_user_mapping(
                        handle.process_data.container.user.as_ref().unwrap(),
                        handle.pid(),
                    );
                    eprintln!("map user ids");
                    ptc_tx.send(MsgParentToChild::UserIdsMapped).unwrap()
                }
                MsgChildToParent::Success => break,
            }
        }

        eprintln!("created2");
        let mut status = -1;
        unsafe { libc::waitpid(handle.pid(), &mut status as *mut _, 0) };
        eprintln!("status {status}");
        handle
    }
}

fn namespace_main<T>(handle: &mut ProcessData<T>) -> i32 {
    eprintln!("TTTTEST");
    let mut is_root = false;
    if let Some(user) = &handle.container.user {
        let _ = handle.ctp_tx.send(MsgChildToParent::MapUserIds);
        let Ok(MsgParentToChild::UserIdsMapped) =
            handle.ptc_rx.recv_timeout(std::time::Duration::new(1, 0))
        else {
            return 1;
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
    (handle.container_main)(&mut handle.main_args)
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

#[derive(Debug)]
enum MsgChildToParent {
    MapUserIds,
    Success,
}

#[derive(Debug)]
enum MsgParentToChild {
    UserIdsMapped,
}
