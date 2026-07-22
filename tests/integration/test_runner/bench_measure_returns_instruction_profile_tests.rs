use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use acton_config::color::ColorMode;

#[test]
fn bench_measure_returns_result_gas_and_instruction_map() {
    ProjectBuilder::new("bench-measure-instruction-profile")
        .test_file(
            "bench_measure",
            r#"
            import "@stdlib/strings"
            import "../../lib/fmt"
            import "../../lib/io"
            import "../../lib/testing/bench"
            import "../../lib/testing/expect"

            fun measuredValue(): int {
                var acc = 0;
                var i = 0;
                while (i < 4) {
                    acc += i * 2;
                    i += 1;
                }
                return acc;
            }

            get fun `test bench measure returns result gas and instruction map`() {
                val profile = bench.measure("loop multiply", measuredValue);

                var instructionCount = 0;
                var sawName = false;
                var sawNonZeroCount = false;
                var sawGas = false;
                var sawCallOverhead = false;
                var boundsAreValid = true;
                var instructionGas: uint64 = 0;
                var r = profile.instructions.findFirst();
                while (r.isFound) {
                    val info = r.loadValue();
                    instructionCount += 1;
                    instructionGas += info.totalGas;
                    sawName = sawName || info.name.calculateLength() > 0;
                    sawNonZeroCount = sawNonZeroCount || info.count > 0;
                    sawGas = sawGas || info.totalGas > 0;
                    sawCallOverhead = sawCallOverhead
                        || info.name.equalTo("CALLDICT")
                        || info.name.equalTo("DICTIGETJMPZ")
                        || info.name.equalTo("DICTPUSHCONST")
                        || info.name.equalTo("SETCP");
                    boundsAreValid = boundsAreValid && info.maxGas >= info.minGas;
                    r = profile.instructions.iterateNext(r);
                }

                println(format("BENCH_RESULT={}", profile.result));
                println(format("BENCH_GAS_POSITIVE={}", profile.gasUsed > 0));
                println(format("BENCH_INSTRUCTIONS_POSITIVE={}", instructionCount > 0));
                println(format("BENCH_NAMES_NON_EMPTY={}", sawName));
                println(format("BENCH_COUNTS_POSITIVE={}", sawNonZeroCount));
                println(format("BENCH_GAS_STATS_POSITIVE={}", sawGas));
                println(format("BENCH_CALL_OVERHEAD_REMOVED={}", !sawCallOverhead));
                println(format("BENCH_BOUNDS_VALID={}", boundsAreValid));
                println(format("BENCH_GAS_MATCHES_INSTRUCTION_TOTAL={}", profile.gasUsed == instructionGas));
                println(format("BENCH_ASM_CODE_NON_EMPTY={}", profile.asmCode.calculateLength() > 0));
                println(format("BENCH_ASM_CODE_NOT_RAW_CALLDICT={}", !profile.asmCode.equalTo("CALLDICT 2\n")));
                println("BENCH_ASM_CODE_START");
                println(StringBuilder.create().append(profile.asmCode).append("BENCH_ASM_CODE_END").build());

                expect(profile.result).toEqual(12);
                expect(profile.gasUsed > 0).toEqual(true);
                expect(profile.gasUsed).toEqual(instructionGas);
                expect(profile.asmCode.calculateLength() > 0).toEqual(true);
                expect(profile.asmCode.equalTo("CALLDICT 2\n")).toEqual(false);
                expect(instructionCount > 0).toEqual(true);
                expect(sawName).toEqual(true);
                expect(sawNonZeroCount).toEqual(true);
                expect(sawGas).toEqual(true);
                expect(sawCallOverhead).toEqual(false);
                expect(boundsAreValid).toEqual(true);
            }
            "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/bench_measure_returns_instruction_profile/bench_measure_returns_result_gas_and_instruction_map.stdout.txt",
        );
}

#[test]
fn bench_format_and_format_diff_return_tables() {
    ProjectBuilder::new("bench-format-tables")
        .test_file(
            "bench_format",
            r#"
            import "@stdlib/strings"
            import "../../lib/io"
            import "../../lib/testing/bench"
            import "../../lib/testing/expect"

            fun baselineValue(): int {
                return 1 + 2;
            }

            fun currentValue(): int {
                var acc = 0;
                var i = 0;
                while (i < 3) {
                    acc += i + 1;
                    i += 1;
                }
                return acc;
            }

            get fun `test bench format and format diff return tables`() {
                val baseline = bench.measure("baseline add", baselineValue);
                val current = bench.measure("current loop", currentValue);

                expect(baseline.result).toEqual(3);
                expect(current.result).toEqual(6);

                val profileTable = bench.format(current);
                val diffTable = bench.formatDiff(baseline, current);

                expect(profileTable.calculateLength() > 0).toEqual(true);
                expect(diffTable.calculateLength() > 0).toEqual(true);

                println(profileTable);
                println(diffTable);
            }
            "#,
        )
        .build()
        .acton()
        .test()
        .color_mode(ColorMode::Always)
        .run()
        .success()
        .assert_passed(1)
        .assert_stdout_svg_snapshot_matches(
            "integration/snapshots/test-runner/bench_measure_returns_instruction_profile/bench_print_and_print_diff_render_tables.stdout.svg",
        );
}

#[test]
fn bench_measure_empty_callback_returns_empty_profile() {
    ProjectBuilder::new("bench-measure-empty-callback")
        .test_file(
            "bench_measure_empty_callback",
            r#"
            import "@stdlib/strings"
            import "../../lib/fmt"
            import "../../lib/io"
            import "../../lib/testing/bench"
            import "../../lib/testing/expect"

            get fun `test bench measure empty callback returns empty profile`() {
                val profile = bench.measure("empty callback", fun() {});

                var instructionCount = 0;
                var instructionGas: uint64 = 0;
                var sawCallOverhead = false;
                var r = profile.instructions.findFirst();
                while (r.isFound) {
                    val info = r.loadValue();
                    instructionCount += 1;
                    instructionGas += info.totalGas;
                    sawCallOverhead = sawCallOverhead
                        || info.name.equalTo("CALLDICT")
                        || info.name.equalTo("DICTIGETJMPZ")
                        || info.name.equalTo("DICTPUSHCONST")
                        || info.name.equalTo("SETCP");
                    r = profile.instructions.iterateNext(r);
                }

                println(format("BENCH_EMPTY_GAS={}", profile.gasUsed));
                println(format("BENCH_EMPTY_INSTRUCTIONS={}", instructionCount));
                println(format("BENCH_EMPTY_CALL_OVERHEAD_REMOVED={}", !sawCallOverhead));
                println(format("BENCH_EMPTY_GAS_MATCHES_INSTRUCTION_TOTAL={}", profile.gasUsed == instructionGas));
                println(bench.format(profile));
                println("BENCH_EMPTY_ASM_CODE_START");
                println(StringBuilder.create().append(profile.asmCode).append("BENCH_EMPTY_ASM_CODE_END").build());

                expect(profile.gasUsed).toEqual(instructionGas);
                expect(sawCallOverhead).toEqual(false);
            }
            "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/bench_measure_returns_instruction_profile/bench_measure_empty_callback_returns_empty_profile.stdout.txt",
        );
}

#[test]
fn bench_measure_reports_callback_exit_code() {
    ProjectBuilder::new("bench-measure-exit-code")
        .test_file(
            "bench_measure_exit_code",
            r#"
            import "../../lib/testing/bench"

            fun failingValue(): int {
                throw 123;
            }

            get fun `test bench measure reports callback exit code`() {
                bench.measure("throwing callback", failingValue);
            }
            "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("bench.measure at ")
        .assert_contains("measured callback exited with exit code 123")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/bench_measure_returns_instruction_profile/bench_measure_reports_callback_exit_code.stdout.txt",
        );
}
