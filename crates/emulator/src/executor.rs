pub type RegisterExtMethodCallback = unsafe extern "C" fn(
    ctx: *mut std::os::raw::c_void,
    arg1: *const std::os::raw::c_char,
) -> *const std::os::raw::c_char;

unsafe extern "C" {
    pub fn create_emulator(
        config: *const std::os::raw::c_char,
        verbosity: std::os::raw::c_int,
    ) -> *mut std::os::raw::c_void;
}
pub type ExtFunc = Option<
    unsafe extern "C" fn(
        ctx: *mut std::os::raw::c_void,
        arg1: *const std::os::raw::c_char,
    ) -> *const std::os::raw::c_char,
>;
unsafe extern "C" {
    pub fn emulate_with_emulator(
        em: *mut std::os::raw::c_void,
        libs: *const std::os::raw::c_char,
        account: *const std::os::raw::c_char,
        message: *const std::os::raw::c_char,
        params: *const std::os::raw::c_char,
    ) -> *mut std::os::raw::c_char;
}
unsafe extern "C" {
    pub fn emulate_sbs(
        em: *mut std::os::raw::c_void,
        libs: *const std::os::raw::c_char,
        account: *const std::os::raw::c_char,
        message: *const std::os::raw::c_char,
        params: *const std::os::raw::c_char,
    ) -> *mut std::os::raw::c_char;
}
unsafe extern "C" {
    pub fn em_sbs_step(em: *mut std::os::raw::c_void) -> bool;
}
unsafe extern "C" {
    pub fn em_sbs_result(em: *mut std::os::raw::c_void) -> *mut std::os::raw::c_char;
    pub fn em_sbs_code_pos(em: *mut std::os::raw::c_void) -> *mut std::os::raw::c_char;
    pub fn em_sbs_stack(em: *mut std::os::raw::c_void) -> *mut std::os::raw::c_char;
    pub fn em_sbs_c7(em: *mut std::os::raw::c_void) -> *mut std::os::raw::c_char;
    pub fn transaction_emulator_sbs_get_control_register(
        tvm: *mut std::os::raw::c_void,
        idx: std::os::raw::c_int,
    ) -> *mut std::os::raw::c_char;
}
unsafe extern "C" {
    pub fn transaction_emulator_register_extmethod(
        transaction_emulator: *mut std::os::raw::c_void,
        id: std::os::raw::c_int,
        ctx: *mut std::os::raw::c_void,
        callback: ExtFunc,
    ) -> *const std::os::raw::c_char;
}
