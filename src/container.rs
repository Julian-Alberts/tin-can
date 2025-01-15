mod builder;
pub mod step;
use std::{io::BufRead, ops::RangeBounds};

pub use builder::*;

use crate::linux;

pub trait MapType {
    fn get_current() -> u32;
    fn file() -> &'static str;
    fn prepare_process(_pid: libc::pid_t) -> std::io::Result<()> {
        Ok(())
    }
    fn subid_file() -> &'static std::path::Path;
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
    fn subid_file() -> &'static std::path::Path {
        std::path::Path::new("/etc/subuid")
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
    fn subid_file() -> &'static std::path::Path {
        std::path::Path::new("/etc/subgid")
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

    pub fn invert(Self { entries, _p }: Self) -> Self {
        Self {
            entries: entries
                .into_iter()
                .map(
                    |IdMapEntry {
                         internal,
                         external,
                         len,
                     }| IdMapEntry {
                        external: internal,
                        internal: external,
                        len,
                    },
                )
                .collect(),
            _p,
        }
    }

    fn is_valid(&self) -> bool {
        let Ok(file) = std::fs::File::open(T::subid_file()) else {
            return false;
        };

        // TODO: Move to linux module
        let uid = unsafe { libc::getuid() };
        let username = std::cell::OnceCell::new();

        let file = std::io::BufReader::new(file);
        let subids_for_user = file
            .lines()
            .filter_map(Result::ok)
            .filter(|entry| !entry.is_empty())
            .filter_map(|entry| {
                let mut parts = entry.split(':');
                let user = parts.next()?;

                if user.chars().all(|c| c.is_ascii_digit()) {
                    if user.parse::<u32>().ok()? != uid {
                        return None;
                    }
                } else {
                    let username = username
                        .get_or_init(|| linux::get_user_name(uid))
                        .as_ref()?;
                    if username != user {
                        return None;
                    }
                };

                let start: u32 = parts.next()?.parse().ok()?;
                let len: u32 = parts.next()?.parse().ok()?;
                Some(start..start + len)
            })
            .collect::<Vec<_>>();

        let allowed = self.entries.iter().all(|mapping| {
            subids_for_user.iter().any(|allowed| {
                allowed.contains(&mapping.internal)
                    && allowed.contains(&(mapping.internal + mapping.len))
            })
        });

        allowed
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct IdMapEntry {
    internal: u32,
    external: u32,
    len: u32,
}
