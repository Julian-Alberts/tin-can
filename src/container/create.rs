use core::panic;
use std::path::PathBuf;

use crate::linux::{self, switch_user, unshare};

use super::*;

#[derive(Debug, thiserror::Error)]
pub enum CreateError {
    #[error("Unshare: {0}")]
    Unshare(#[from] linux::UnshareError),
    #[error("Switch User: {0}")]
    SwitchUser(#[from] linux::SwitchUserError),
}

impl Container<Validated> {
    pub fn create<F: FnOnce() -> T + Send + 'static, T: Send + 'static>(self, f: F) {
        let (ctp_tx, ctp_rx) = std::sync::mpsc::channel();
        let (ptc_tx, ptc_rx) = std::sync::mpsc::channel();

        let pid = linux::clone3(libc::CLONE_NEWUSER).unwrap();

        if pid == 0 {}

        while let Ok(msg) = ctp_rx.recv() {
            match msg {
                MsgChildToParent::UnsharedUserNamespace((user, tid)) => {
                    create_user_mapping(&user, tid);
                    ptc_tx.send(MsgParentToChild::User(user)).unwrap()
                }
                MsgChildToParent::Success => break,
            }
        }
    }
}

fn container_main<F: FnOnce()>(
    ctp_tx: std::sync::mpsc::Sender<MsgChildToParent>,
    ptc_rx: std::sync::mpsc::Receiver<MsgParentToChild>,
    Container {
        user,
        mount,
        pid,
        net,
        _p,
    }: Container<Validated>,
    f: F,
) {
    let mut is_root = false;
    if let Some(user) = user {
        let _ = ctp_tx.send(MsgChildToParent::UnsharedUserNamespace((user, unsafe {
            libc::gettid()
        })));
        let Ok(MsgParentToChild::User(user)) = ptc_rx.recv() else {
            panic!("Unexpected message")
        };
        if user.uid_map.iter().any(|m| m.start_intern == 0)
            && user.gid_map.iter().any(|m| m.start_intern == 0)
        {
            switch_user((0, 0));
            is_root = true;
        }
    }
    ctp_tx.send(MsgChildToParent::Success).unwrap();
    (f)();
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
    UnsharedUserNamespace((UserNamespace<Validated>, libc::pid_t)),
    Success,
}
enum MsgParentToChild {
    User(UserNamespace<Validated>),
}
