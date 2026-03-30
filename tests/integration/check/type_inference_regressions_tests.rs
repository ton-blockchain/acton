use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

#[test]
#[named]
fn test_check_does_not_hang_on_recursive_self_receiver_deduction() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract(
            "main",
            r"
            struct BigArray<T> {
                arr: [array<array<T>>, int] = [array<array<T>> [], 0]
            }

            fun BigArray<T>.push(mutate self, value: T) {
                var [topLevel, itemsCount] = self.arr;

                if (itemsCount >= 65025) {
                    throw 5;
                }

                val binIdx = itemsCount / 255;
                while (topLevel.size() <= binIdx) {
                    topLevel.push();
                }

                var bin = topLevel.get(binIdx);
                bin.push(value);
                topLevel.set(bin, binIdx);
                itemsCount += 1;

                self.arr = [topLevel, itemsCount];
            }
        ",
        )
        .with_lint_level("explicit-return-type", "allow")
        .build();

    project.acton().init().run().success();
    let output = project.acton().check().run().success();
    assert!(
        output.get_normalized_stderr().is_empty(),
        "expected no diagnostics for regression fixture, got:\n{}",
        output.get_normalized_stderr()
    );
}
