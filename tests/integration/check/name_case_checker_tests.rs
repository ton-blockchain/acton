use crate::integration::check::run_fix_test;
use crate::integration::check::run_simple_test;
use crate::support::project::ProjectBuilder;
use function_name::named;

#[test]
#[named]
fn test_check_name_case_checker_globals() {
    run_simple_test(
        "name_case_checker",
        r#"
            struct low_struct {
                Bad_field: int,
            }

            enum low_enum {
                low_member = 1,
            }

            type low_alias = low_struct

            const badConst = 2
            global Bad_global: int

            fun Bad_function(value: int): int {
                return value + badConst;
            }

            fun low_struct.Bad_method(self): int {
                return self.Bad_field + Bad_global;
            }

            fun useAlias(x: low_alias): int {
                return x.Bad_field;
            }

            fun main(): int {
                val instance = low_struct { Bad_field: 10 };
                val aliasValue: low_alias = instance;
                val enumValue = low_enum.low_member;

                Bad_global = 5;
                val fromFunction = Bad_function(enumValue as int);
                val fromMethod = instance.Bad_method();
                val fromAlias = useAlias(aliasValue);
                return fromFunction + fromMethod + fromAlias + badConst;
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_name_case_checker_locals_and_type_parameters() {
    run_simple_test(
        "name_case_checker",
        r#"
            fun localHelper<bad_type>(Bad_param: bad_type): bad_type {
                val Bad_local = Bad_param;
                return Bad_local;
            }

            fun main(): int {
                return localHelper<int>(10);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_name_case_checker_ignores_internal_names_and_get_methods() {
    run_simple_test(
        "name_case_checker",
        r#"
            struct _hidden_struct {
                _hidden_field: int,
            }

            const _hidden_const = 3
            global _hidden_global: int

            fun _hidden_fun(): int {
                return 1;
            }

            fun _hidden_struct._hidden_method(self): int {
                return self._hidden_field;
            }

            get fun get_wallet_info(): int {
                return 2;
            }

            fun main(): int {
                _hidden_global = 4;

                val fromFun = _hidden_fun();
                val fromMethod = _hidden_struct { _hidden_field: fromFun }._hidden_method();
                return fromFun + fromMethod + get_wallet_info() + _hidden_const + _hidden_global;
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_fix_name_case_checker_locals_and_type_parameters() {
    run_fix_test(
        r#"
            fun localHelper<bad_type>(Bad_param: bad_type): bad_type {
                val Bad_local = Bad_param;
                return Bad_local;
            }

            fun main(): int {
                return localHelper<int>(10);
            }
        "#,
        r#"
            fun localHelper<BadType>(badParam: BadType): BadType {
                val badLocal = badParam;
                return badLocal;
            }

            fun main(): int {
                return localHelper<int>(10);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_fix_name_case_checker_globals_and_usages() {
    run_fix_test(
        r#"
            struct low_struct {
                Bad_field: int,
            }

            enum low_enum {
                low_member = 1,
            }

            type low_alias = low_struct

            const badConst = 2
            global Bad_global: int

            fun Bad_function(value: int): int {
                return value + badConst;
            }

            fun low_struct.Bad_method(self): int {
                return self.Bad_field + Bad_global;
            }

            fun useAlias(x: low_alias): int {
                return x.Bad_field;
            }

            fun main(): int {
                val instance = low_struct { Bad_field: 10 };
                val aliasValue: low_alias = instance;
                val enumValue = low_enum.low_member;

                Bad_global = 5;
                val fromFunction = Bad_function(enumValue as int);
                val fromMethod = instance.Bad_method();
                val fromAlias = useAlias(aliasValue);
                return fromFunction + fromMethod + fromAlias + badConst;
            }
        "#,
        r#"
            struct LowStruct {
                badField: int,
            }

            enum LowEnum {
                LowMember = 1,
            }

            type LowAlias = LowStruct

            const BAD_CONST = 2
            global badGlobal: int

            fun badFunction(value: int): int {
                return value + BAD_CONST;
            }

            fun LowStruct.badMethod(self): int {
                return self.badField + badGlobal;
            }

            fun useAlias(x: LowAlias): int {
                return x.badField;
            }

            fun main(): int {
                val instance = LowStruct { badField: 10 };
                val aliasValue: LowAlias = instance;
                val enumValue = LowEnum.LowMember;

                badGlobal = 5;
                val fromFunction = badFunction(enumValue as int);
                val fromMethod = instance.badMethod();
                val fromAlias = useAlias(aliasValue);
                return fromFunction + fromMethod + fromAlias + BAD_CONST;
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_fix_name_case_checker_struct_literal_field_key_usage() {
    run_fix_test(
        r#"
            struct SomeStruct {
                Bad_field: int,
            }

            fun main(): int {
                val sourceValue = 10;
                val item = SomeStruct { Bad_field: sourceValue };
                return item.Bad_field;
            }
        "#,
        r#"
            struct SomeStruct {
                badField: int,
            }

            fun main(): int {
                val sourceValue = 10;
                val item = SomeStruct { badField: sourceValue };
                return item.badField;
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_fix_name_case_checker_updates_usages_in_another_file() {
    let project = ProjectBuilder::new(&format!("check-fix-{}", function_name!()))
        .contract(
            "main",
            r#"
                import "./api.tolk";
                import "./other.tolk";

                fun main(): int {
                    return Bad_fn() + otherMain();
                }
            "#,
        )
        .file(
            "contracts/api",
            r#"
                fun Bad_fn(): int {
                    return 1;
                }
            "#,
        )
        .file(
            "contracts/other",
            r#"
                import "./api.tolk";

                fun otherMain(): int {
                    return Bad_fn();
                }
            "#,
        )
        .build();

    project.acton().init().run().success();
    project.acton().check().arg("--fix").run().success();

    let main_path = project.path().join("contracts/main.tolk");
    let api_path = project.path().join("contracts/api.tolk");
    let other_path = project.path().join("contracts/other.tolk");

    let main_actual = std::fs::read_to_string(&main_path)
        .unwrap_or_else(|e| panic!("failed to read fixed file '{}': {}", main_path.display(), e));
    let api_actual = std::fs::read_to_string(&api_path)
        .unwrap_or_else(|e| panic!("failed to read fixed file '{}': {}", api_path.display(), e));
    let other_actual = std::fs::read_to_string(&other_path).unwrap_or_else(|e| {
        panic!(
            "failed to read fixed file '{}': {}",
            other_path.display(),
            e
        )
    });

    let expected_main = r#"
                import "./api.tolk";
                import "./other.tolk";

                fun main(): int {
                    return badFn() + otherMain();
                }
            "#;
    let expected_api = r#"
                fun badFn(): int {
                    return 1;
                }
            "#;
    let expected_other = r#"
                import "./api.tolk";

                fun otherMain(): int {
                    return badFn();
                }
            "#;

    assert_eq!(
        main_actual.trim(),
        expected_main.trim(),
        "fixed file content mismatch for {}",
        main_path.display()
    );
    assert_eq!(
        api_actual.trim(),
        expected_api.trim(),
        "fixed file content mismatch for {}",
        api_path.display()
    );
    assert_eq!(
        other_actual.trim(),
        expected_other.trim(),
        "fixed file content mismatch for {}",
        other_path.display()
    );
}
