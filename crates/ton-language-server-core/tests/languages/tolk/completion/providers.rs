#![allow(clippy::needless_raw_string_hashes)]

#[path = "../../../support/mod.rs"]
mod common;

mod providers {
    mod acton_contract_id;
    mod acton_get_method;
    mod acton_wallet_name;
    mod annotations;
    mod contract_fields;
    mod entry_points;
    mod enum_declaration;
    mod enum_values;
    mod expression_snippets;
    mod field_init;
    mod field_modifiers;
    mod function_names;
    mod import_path;
    mod index_access;
    mod keywords;
    mod match_arms;
    mod profiling;
    mod references;
    mod returns;
    mod statement_snippets;
    mod storage;
    mod support;
    mod throw_assert;
    mod top_level;
    mod variable_size_types;
}
