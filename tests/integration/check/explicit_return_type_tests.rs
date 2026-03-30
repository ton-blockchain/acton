use crate::integration::check::{run_rule_fix_test, run_rule_test};
use function_name::named;

const RULE_CODE: &str = "S002";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

fn run_fix_test(before: &str, after: &str, name: &str) {
    run_rule_fix_test(RULE_CODE, before, after, name);
}

#[test]
#[named]
fn test_check_explicit_return_type_with_explicit_type() {
    run_simple_test(
        "explicit_return_type",
        r"
            fun function(): slice {
                return beginCell().toSlice();
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_explicit_return_type_skips_contract_entrypoints() {
    run_simple_test(
        "explicit_return_type",
        r"
            fun main() {}
            fun onInternalMessage() {}
            fun onExternalMessage() {}
            fun onRunTickTock() {}
            fun onSplitPrepare() {}
            fun onSplitInstall() {}
            fun onBouncedMessage(_: InMessageBounced) {}
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_explicit_return_type() {
    run_fix_test(
        r"
            fun function() {
                return beginCell().toSlice();
            }
        ",
        r"
            fun function(): slice {
                return beginCell().toSlice();
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_explicit_return_type() {
    run_simple_test(
        "explicit_return_type",
        r"
            fun function() {
                return beginCell().toSlice();
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_explicit_return_type_for_method() {
    run_fix_test(
        r"
            struct Counter {
                value: int,
            }

            fun Counter.getValue(self) {
                return self.value;
            }
        ",
        r"
            struct Counter {
                value: int,
            }

            fun Counter.getValue(self): int {
                return self.value;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_explicit_return_type_for_method() {
    run_simple_test(
        "explicit_return_type",
        r"
            struct Counter {
                value: int,
            }

            fun Counter.getValue(self) {
                return self.value;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_explicit_return_type_for_get_method() {
    run_fix_test(
        r"
            get fun getCounter() {
                return 42;
            }
        ",
        r"
            get fun getCounter(): int {
                return 42;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_explicit_return_type_for_get_method() {
    run_simple_test(
        "explicit_return_type",
        r"
            get fun getCounter() {
                return 42;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_explicit_return_type_for_void() {
    run_fix_test(
        r"
            fun doNothing() {
            }
        ",
        r"
            fun doNothing() {
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_explicit_return_type_for_void() {
    run_simple_test(
        "explicit_return_type",
        r"
            fun doNothing() {
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_explicit_return_type_for_struct() {
    run_fix_test(
        r"
            struct Payload {
                value: int,
            }

            fun buildPayload() {
                return Payload { value: 10 };
            }
        ",
        r"
            struct Payload {
                value: int,
            }

            fun buildPayload(): Payload {
                return Payload { value: 10 };
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_explicit_return_type_for_struct() {
    run_simple_test(
        "explicit_return_type",
        r"
            struct Payload {
                value: int,
            }

            fun buildPayload() {
                return Payload { value: 10 };
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_explicit_return_type_for_generic() {
    run_fix_test(
        r"
            fun identity<T>(value: T) {
                return value;
            }
        ",
        r"
            fun identity<T>(value: T): T {
                return value;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_explicit_return_type_for_generic() {
    run_simple_test(
        "explicit_return_type",
        r"
            fun identity<T>(value: T) {
                return value;
            }
        ",
        function_name!(),
    );
}
