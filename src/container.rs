mod builder;
pub mod step;
pub use builder::*;

pub trait Step {
    type Error: std::error::Error;
    type Ok;
    fn run(self) -> Result<Self::Ok, Self::Error>;
}

pub trait MapType {
    fn get_current() -> u32;
    fn file() -> &'static str;
    fn prepare_process(_pid: libc::pid_t) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct User;
impl MapType for User {
    fn get_current() -> u32 {
        crate::linux::get_euid()
    }

    fn file() -> &'static str {
        "uid_map"
    }
}
#[derive(Debug)]
pub struct Group;
impl MapType for Group {
    fn get_current() -> u32 {
        crate::linux::get_egid()
    }

    fn file() -> &'static str {
        "gid_map"
    }

    fn prepare_process(_pid: libc::pid_t) -> std::io::Result<()> {
        let mut path = std::path::PathBuf::from("/proc");
        path.push(_pid.to_string());
        path.push("setgroups");
        std::fs::write(path, "deny")
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct IdMap<T> {
    entries: Vec<IdMapEntry>,
    _p: std::marker::PhantomData<T>,
}

impl<T> IdMap<T>
where
    T: MapType,
{
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            _p: std::marker::PhantomData,
        }
    }

    pub fn new_with_current_user_as_root() -> Self {
        let mut map = Self {
            entries: Vec::new(),
            _p: std::marker::PhantomData,
        };
        let ext_id = T::get_current();
        map.add(0, ext_id, 1);
        map
    }

    pub fn add(&mut self, internal: u32, external: u32, len: u32) {
        self.entries.push(IdMapEntry {
            internal,
            external,
            len,
        });
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct IdMapEntry {
    internal: u32,
    external: u32,
    len: u32,
}
