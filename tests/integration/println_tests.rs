use crate::support::assertions::TestOutputExt;
use crate::support::project::ProjectBuilder;
use acton_config::color::ColorMode;

#[test]
fn test_println_big_array() {
    let project = ProjectBuilder::new("println-big-array")
        .script_file(
            "main",
            r#"
            import "../../lib/io"
            import "../../lib/types/big_array"

            struct Point {
                x: int
                y: int
            }

            struct Collection {
                values: BigArray<int>?
                next: int
            }

            type Numbers = BigArray<int>

            fun main() {
                println(BigArray<int>.createEmpty());
                val numbers: Numbers = BigArray<int>.createFromArray([1, 2, 3]);
                println(numbers);
                println("values={}", numbers);
                println(Collection { values: numbers, next: 42 });
                println(Collection { values: null, next: 43 });
                println([numbers, BigArray<int>.createEmpty()]);
                println(BigArray<Point>.createFromArray([
                    Point { x: 1, y: 2 }, Point { x: 3, y: 4 }
                ]));
                println(BigArray<(int, bool)>.createFromArray([(1, true), (2, false)]));
                println(BigArray<array<int>>.createFromArray([[1, 2], [], [3]]));
                println(BigArray<BigArray<int>>.createFromArray([
                    numbers, BigArray<int>.createEmpty()
                ]));
            }
            "#,
        )
        .build();

    project
        .acton()
        .script("scripts/main.tolk")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/println/test_println_big_array.stdout.txt");
}

#[test]
fn test_println_big_array_chunk_boundaries() {
    let project = ProjectBuilder::new("println-big-array-chunk-boundaries")
        .test_file(
            "big_array",
            r#"
            import "../../lib/io"
            import "../../lib/fmt"
            import "../../lib/testing/expect"
            import "../../lib/types/big_array"

            get fun `test big array chunk boundaries`() {
                var values = BigArray<int>.createEmpty();
                var expected = "";
                repeat (511) {
                    val index = values.size();
                    if (index > 0) {
                        expected = format("{}, {}", expected, index);
                    } else {
                        expected = format("{}", index);
                    }
                    values.push(index);
                    if (values.size() == 255 || values.size() == 256 ||
                        values.size() == 510 || values.size() == 511) {
                        expect(format("{}", values)).toEqual(format("BigArray<int> [{}]", expected));
                    }
                }
                repeat (256) {
                    values.pop();
                }
                values.push(999);
                println(values);
                repeat (256) {
                    values.pop();
                }
                println(values);
            }
            "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/println/test_println_big_array_chunk_boundaries.stdout.txt",
        );
}

#[test]
fn test_println_single_tuple_field_and_unrelated_big_array() {
    let project = ProjectBuilder::new("println-single-tuple-field")
        .script_file(
            "main",
            r#"
            import "../../lib/io"

            struct Box<T> {
                value: T
            }

            struct BigArray<T> {
                items: array<T>
            }

            fun main() {
                println(Box<[int, bool]> { value: [1, true] });
                println(Box { value: array<int> [1, 2, 3] });
                println(Box { value: array<int> [] });
                println(Box { value: Box<[int, bool]> { value: [2, false] } });
                println(BigArray<int> { items: [4, 5] });
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
            "integration/snapshots/println/test_println_single_tuple_field_and_unrelated_big_array.stdout.txt",
        );
}

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
            "integration/snapshots/println/test_println_formatting.stdout.svg",
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
            "integration/snapshots/println/test_println_tuples_and_tensors.stdout.svg",
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
            "integration/snapshots/println/test_println_nesting_complex.stdout.svg",
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
            "integration/snapshots/println/test_println_various_slices.stdout.svg",
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
            "integration/snapshots/println/test_println_typed_cell_includes_decoded_value.stdout.txt",
        );
}

#[test]
fn test_println_generic_type_coloring_keeps_punctuation_plain() {
    let project = ProjectBuilder::new("println-generic-type-colors")
        .script_file(
            "main",
            r#"
            import "../../lib/io"

            struct Leaf {
                value: uint8
            }

            struct GenericBox {
                typedCell: Cell<Leaf>
                boxedItems: map<uint8, Cell<Leaf>>
            }

            fun main() {
                val leafCell = Leaf { value: 7 }.toCell() as Cell<Leaf>;
                var boxedItems = createEmptyMap<uint8, Cell<Leaf>>();
                boxedItems.set(1 as uint8, leafCell);
                println(GenericBox { typedCell: leafCell, boxedItems });
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
            "integration/snapshots/println/test_println_generic_type_coloring_keeps_punctuation_plain.stdout.svg",
        );
}
