trait NsType {
    fn file() -> &'static str;
}

macro_rules! ns_type {
    ($name:ident => $file:literal) => {
        #[derive(Debug)]
        pub struct $name;
        impl NsType for $name {
            fn file() -> &'static str {
                $file
            }
        }
    };
}

ns_type!(CGroup => "cgroup");
ns_type!(Ipc => "ipc");
ns_type!(Mnt => "mnt");
ns_type!(Net => "net");
ns_type!(Pid => "pid");
ns_type!(Time => "time");
ns_type!(User => "user");
ns_type!(Uts => "uts");

#[derive(Debug, Default)]
pub struct Context {
    cgroup: Option<Namespace<CGroup>>,
    ipc: Option<Namespace<Ipc>>,
    mnt: Option<Namespace<Mnt>>,
    net: Option<Namespace<Net>>,
    pid: Option<Namespace<Pid>>,
    time: Option<Namespace<Time>>,
    user: Option<Namespace<User>>,
    uts: Option<Namespace<Uts>>,
}

impl Context {
    pub fn set_cgroup(&mut self) -> std::io::Result<()> {
        self.cgroup = Some(Namespace::new()?);
        Ok(())
    }

    pub fn set_ipc(&mut self) -> std::io::Result<()> {
        self.ipc = Some(Namespace::new()?);
        Ok(())
    }

    pub fn entered_mnt_ns(&mut self) -> std::io::Result<()> {
        self.mnt = Some(Namespace::new()?);
        Ok(())
    }

    pub fn set_net(&mut self) -> std::io::Result<()> {
        self.net = Some(Namespace::new()?);
        Ok(())
    }

    pub fn entered_pid_ns(&mut self) -> std::io::Result<()> {
        self.pid = Some(Namespace::new()?);
        Ok(())
    }

    pub fn set_time(&mut self) -> std::io::Result<()> {
        self.time = Some(Namespace::new()?);
        Ok(())
    }

    pub fn entered_user_ns(&mut self) -> std::io::Result<()> {
        self.user = Some(Namespace::new()?);
        Ok(())
    }

    pub fn set_uts(&mut self) -> std::io::Result<()> {
        self.uts = Some(Namespace::new()?);
        Ok(())
    }

    pub fn cgroup(&mut self) -> Option<&Namespace<CGroup>> {
        self.cgroup.as_ref()
    }

    pub fn ipc(&self) -> Option<&Namespace<Ipc>> {
        self.ipc.as_ref()
    }

    pub fn mnt(&self) -> Option<&Namespace<Mnt>> {
        self.mnt.as_ref()
    }

    pub fn net(&self) -> Option<&Namespace<Net>> {
        self.net.as_ref()
    }

    pub fn pid(&self) -> Option<&Namespace<Pid>> {
        self.pid.as_ref()
    }

    pub fn time(&self) -> Option<&Namespace<Time>> {
        self.time.as_ref()
    }

    pub fn user(&self) -> Option<&Namespace<User>> {
        self.user.as_ref()
    }

    pub fn uts(&self) -> Option<&Namespace<Uts>> {
        self.uts.as_ref()
    }
}

#[derive(Debug)]
pub struct Namespace<T>
where
    T: NsType,
{
    /// This is only used to keep the namespace alive after the command finished
    _ns_file: std::fs::File,
    _p: std::marker::PhantomData<T>,
}

impl<T> Namespace<T>
where
    T: NsType,
{
    pub fn new() -> std::io::Result<Self> {
        let pid = std::process::id();
        let path = format!("/proc/{pid}/ns/{ns_type}", ns_type = T::file());
        let path = std::path::PathBuf::from(path);
        Ok(Self {
            _ns_file: std::fs::File::open(path)?,
            _p: std::marker::PhantomData,
        })
    }
}
