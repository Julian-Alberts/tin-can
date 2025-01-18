use crate::linux;

#[derive(Default, Debug)]
pub struct Context {
    cgroup: bool,
    ipc: bool,
    mnt: bool,
    net: bool,
    pid: bool,
    time: bool,
    user: bool,
    uts: bool,
    pid_fd: Option<std::fs::File>,
}

impl Context {
    pub fn set_cgroup(&mut self) {
        self.cgroup = true;
        self.set_pidfd();
    }

    pub fn set_ipc(&mut self) {
        self.ipc = true;
        self.set_pidfd();
    }

    pub fn entered_mnt_ns(&mut self) {
        self.mnt = true;
        self.set_pidfd();
    }

    pub fn set_net(&mut self) {
        self.net = true;
        self.set_pidfd();
    }

    pub fn entered_pid_ns(&mut self) {
        self.pid = true;
        self.set_pidfd();
    }

    pub fn set_time(&mut self) {
        self.time = true;
        self.set_pidfd();
    }

    pub fn entered_user_ns(&mut self) {
        self.user = true;
        self.set_pidfd();
    }

    pub fn set_uts(&mut self) {
        self.uts = true;
        self.set_pidfd();
    }

    fn set_pidfd(&mut self) {
        let pidfd = linux::pidfd_open(std::process::id());
        self.pid_fd = Some(pidfd);
    }

    pub fn cgroup(&mut self) -> bool {
        self.cgroup
    }

    pub fn ipc(&self) -> bool {
        self.ipc
    }

    pub fn mnt(&self) -> bool {
        self.mnt
    }

    pub fn net(&self) -> bool {
        self.net
    }

    pub fn pid(&self) -> bool {
        self.pid
    }

    pub fn time(&self) -> bool {
        self.time
    }

    pub fn user(&self) -> bool {
        self.user
    }

    pub fn uts(&self) -> bool {
        self.uts
    }
}
