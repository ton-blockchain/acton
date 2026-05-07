use crate::support::assertions::TestOutputExt;
use crate::support::project::ProjectBuilder;
use acton_config::color::ColorMode;

#[test]
fn test_println_formatting() {
    let project = ProjectBuilder::new("println-formatting")
        .script_file(
            "main",
            r#"
            import "../../lib/io"

            struct Simple {
                a: int,
                b: bool,
            }

            struct Nested {
                s: Simple,
                addr: address,
                opt: int?,
                str: string,
            }

            fun main() {
                // 1. Primitives
                println(123);
                println(true);
                println(null);
                println("plain string");

                // 2. Simple struct
                println(Simple { a: 1, b: true });

                // 3. Nested struct
                println(Nested {
                    s: Simple { a: 42, b: false },
                    addr: address("EQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHByMJ"),
                    opt: 7,
                    str: "some string",
                });

                // 4. Nullable value
                println(null as Nested?);

                // 5. Cell
                val c = beginCell().storeUint(0x12345678, 32).endCell();
                println(c);
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/main.tolk")
        .keep_color_env()
        .color_mode(ColorMode::Always)
        .run()
        .success()
        .assert_stdout_svg_snapshot_matches(
            "integration/snapshots/test_println_formatting.stdout.svg",
        );
}

#[test]
fn test_println_tuples_and_tensors() {
    let project = ProjectBuilder::new("println-tuples-tensors")
        .script_file(
            "main",
            r#"
            import "../../lib/io"

            fun main() {
                // 1. Tensors
                println((1, 2, 3));

                // 2. Tuples
                println([1, 2, 3, 4, 5]);

                // 3. Nested tuples and tensors
                println((
                    (1, 2),
                    [3, 4]
                ));

                println([
                    [1, 2, 3],
                    [4, 5, 6]
                ]);
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/main.tolk")
        .keep_color_env()
        .color_mode(ColorMode::Always)
        .run()
        .success()
        .assert_stdout_svg_snapshot_matches(
            "integration/snapshots/test_println_tuples_and_tensors.stdout.svg",
        );
}

#[test]
fn test_println_nesting_complex() {
    let project = ProjectBuilder::new("println-nesting-complex")
        .script_file(
            "main",
            r#"
            import "../../lib/io"

            struct Inner {
                x: int,
                y: int,
            }

            struct Outer {
                inner: Inner,
                tags: [string, string],
                maybe_val: int?,
            }

            fun main() {
                val o = Outer {
                    inner: Inner { x: 10, y: 20 },
                    tags: ["tag1", "tag2"],
                    maybe_val: 42,
                };
                println(o);

                val o2 = Outer {
                    inner: Inner { x: 0, y: 0 },
                    tags: ["empty", ""],
                    maybe_val: null,
                };
                println(o2);
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/main.tolk")
        .keep_color_env()
        .color_mode(ColorMode::Always)
        .run()
        .success()
        .assert_stdout_svg_snapshot_matches(
            "integration/snapshots/test_println_nesting_complex.stdout.svg",
        );
}

#[test]
fn test_println_various_slices() {
    let project = ProjectBuilder::new("println-slices")
        .script_file(
            "main",
            r#"
            import "../../lib/io"

            fun main() {
                // 1. Snake strings
                println("hello world");

                // 2. Slice with some data
                val s1 = beginCell().storeUint(0xABCDEF, 24).endCell().beginParse();
                println((s1, s1));

                // 3. Empty slice
                val s2 = beginCell().endCell().beginParse();
                println(("empty", s2));
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/main.tolk")
        .keep_color_env()
        .color_mode(ColorMode::Always)
        .run()
        .success()
        .assert_stdout_svg_snapshot_matches(
            "integration/snapshots/test_println_various_slices.stdout.svg",
        );
}

#[test]
fn test_println_typed_cell_includes_decoded_value() {
    let project = ProjectBuilder::new("println-typed-cell-decoded")
        .script_file(
            "main",
            r#"
            import "../../lib/io"

            struct Child {
                value: uint8
            }

            struct Boxed {
                child: Cell<Child>
            }

            fun main() {
                val child = Child { value: 42 }.toCell() as Cell<Child>;
                println(Boxed { child });
            }
        "#,
        )
        .build();

    project
        .acton()
        .script("scripts/main.tolk")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_println_typed_cell_includes_decoded_value.stdout.txt",
        );
}
