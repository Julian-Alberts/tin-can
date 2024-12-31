mod capability;

#[derive(Debug, Clone, Copy)]
pub enum Capability {
    SETGID,
    SETUID,
}

impl Capability {
    fn to_cap(&self) -> capability::cap_value_t {
        match self {
            Capability::SETGID => capability::CAP_SETGID as i32,
            Capability::SETUID => capability::CAP_SETUID as i32,
        }
    }
}

pub fn has_capability(cap: Capability) -> bool {
    let caps = unsafe { capability::cap_get_proc() };
    let mut cap_value = 0;
    unsafe {
        capability::cap_get_flag(
            caps,
            cap.to_cap(),
            capability::cap_flag_t_CAP_EFFECTIVE,
            &mut cap_value,
        )
    };
    cap_value == 1
}
