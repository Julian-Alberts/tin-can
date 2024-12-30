fn main() {
    let mut data: (&str, fn(), Option<_>) = ("Testdata", || println!("inner"), None);
    let code = clone(container_main, &mut data);
    println!("Result {:?}", data.2);
    std::process::exit(code)
}

fn container_main(data: &mut (&str, fn(), Option<i32>)) -> i32 {
    println!("running process with data {}", data.0);
    data.2 = Some(123);
    (data.1)();
    0
}

fn clone<T>(callback: fn(&mut T) -> i32, args: &mut T) -> i32 {
    let child_stack = new_stack();

    struct Args<'a, T> {
        callback: fn(&mut T) -> i32,
        data: &'a mut T,
    }
    extern "C" fn cb<T>(args: *mut libc::c_void) -> i32 {
        println!("Test");
        let args = unsafe { (args as *mut Args<T>).as_mut().unwrap() };
        (args.callback)(args.data)
    }

    let mut args = Args {
        callback,
        data: args,
    };

    println!("1");
    let res = unsafe {
        libc::clone(
            cb::<T>,
            child_stack,
            libc::CLONE_VM | libc::CLONE_VFORK,
            &mut args as *mut _ as *mut libc::c_void,
        )
    };

    if res == -1 {
        panic!("Failed to spawn new process");
    }

    unsafe { libc::waitpid(res, std::ptr::null_mut(), 0) }
}

fn new_stack() -> *mut libc::c_void {
    const STACK_SIZE: libc::size_t = 1024 * 1024;
    let ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            1024 * 1024,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_STACK,
            -1,
            0,
        )
    };
    unsafe { ptr.add(STACK_SIZE) }
}
