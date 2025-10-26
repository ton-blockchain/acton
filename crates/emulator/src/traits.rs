pub trait BaseExecutor {
    fn step(&self) -> bool;
    fn register_ext_method(
        &mut self,
        id: i32,
        ctx: *mut std::os::raw::c_void,
        callback: RegisterExtMethodCallback,
    );
}

pub type RegisterExtMethodCallback = unsafe extern "C" fn(
    ctx: *mut std::os::raw::c_void,
    arg1: *const std::os::raw::c_char,
) -> *const std::os::raw::c_char;
