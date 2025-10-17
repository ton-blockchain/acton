use crate::executor::Executor;
use crate::exts_lib::Tuple;
use crate::stack_serialization::TupleItem;
use crate::{TESTS, extension, pop_args, register_ext_methods};
use core::ffi::c_char;

extension!(print, (s: String), |_stack: &mut Tuple, (s,)| {
    println!("{}", s);
});

extension!(eprint, (s: String), |_stack: &mut Tuple, (s,)| {
    eprintln!("{}", s);
});

extension!(read_file, (path: String), |stack: &mut Tuple, (path,)| {
    match std::fs::read_to_string(&path) {
        Ok(content) => stack.push_string(&content),
        Err(_) => stack.push(TupleItem::Null),
    }
});

extension!(assert_equal, (left: Tuple, right: Tuple), |stack: &mut Tuple, (left, right)| {
    if left == right {
        stack.push_bool_as_int(true);
    } else {
        stack.push_bool_as_int(false);
    }
});

extension!(register_test, (name: String), |_stack: &mut Tuple, (name,)| {
    TESTS.lock().unwrap().push(name);
});

pub fn register_extensions(executor: &mut Executor) {
    register_ext_methods!(executor, {
        1 => print,
        2 => eprint,
        3 => read_file,
        4 => assert_equal,
        5 => register_test,
    });
}
