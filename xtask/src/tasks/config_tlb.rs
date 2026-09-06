use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use clap::Args;
use serde::Serialize;
use tlb_syntax::{AstNode, Declaration};
use ton_syntax::ast::PreorderTraverse;

/// Extracts configuration schemas for Explorer's build-time TL-B catalog.
/// The caller owns source revision metadata and writing the generated artifact.
#[derive(Args)]
pub(crate) struct ConfigTlbArgs {
    /// Pinned block.tlb used by both the decoder and the schema viewer.
    source: PathBuf,
}

#[derive(Debug, Serialize)]
struct ParameterSchema {
    line: usize,
    roots: Vec<usize>,
    dependencies: Vec<Dependency>,
}

#[derive(Debug, Serialize)]
struct Dependency {
    declaration: usize,
    depth: usize,
    parameterized: bool,
    #[serde(rename = "typeName")]
    type_name: String,
}

#[derive(Debug, Serialize)]
struct Catalog {
    parameters: BTreeMap<u32, ParameterSchema>,
    declarations: BTreeMap<usize, String>,
}

/// Writes only JSON to stdout so the frontend generator can consume it directly.
/// Parse and extraction failures retain the input path in the command's error.
pub(crate) fn run(args: ConfigTlbArgs) -> Result<()> {
    let source = fs::read_to_string(&args.source)
        .with_context(|| format!("failed to read TL-B source {}", args.source.display()))?;
    let catalog = generate_catalog(&source).with_context(|| {
        format!(
            "failed to extract config TL-B from {}",
            args.source.display()
        )
    })?;

    println!("{}", serde_json::to_string(&catalog)?);
    Ok(())
}

/// Uses syntax ranges to retain tags, constraints and cell references verbatim.
/// Only whitespace between top-level fields is formatted for the code viewer.
fn generate_catalog(source: &str) -> Result<Catalog> {
    let file = tlb_syntax::parse(source)?;
    ensure!(!file.has_errors(), "TL-B source contains syntax errors");
    let declarations: Vec<_> = file
        .program()
        .context("TL-B source has no program")?
        .declarations()
        .collect();
    let mut types: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    let mut roots: BTreeMap<u32, Vec<usize>> = BTreeMap::new();

    for (index, declaration) in declarations.iter().enumerate() {
        let result = declaration.combinator().context("missing result type")?;
        let name = result
            .name()
            .context("missing result type name")?
            .text(source);
        types.entry(name).or_default().push(index);

        if name == "ConfigParam" {
            let parameters: Vec<_> = result.params().collect();
            ensure!(
                parameters.len() == 1,
                "ConfigParam must have one literal ID"
            );
            let id = parameters[0]
                .text(source)
                .parse::<u32>()
                .context("ConfigParam ID must be a nonnegative integer literal")?;
            roots.entry(id).or_default().push(index);
        }
    }

    ensure!(
        !roots.is_empty(),
        "TL-B source contains no ConfigParam declarations"
    );
    let references: Vec<_> = declarations
        .iter()
        .map(|declaration| referenced_types(*declaration, source))
        .collect();
    let mut parameters = BTreeMap::new();
    let mut used = BTreeSet::<usize>::new();

    for (id, root_ids) in roots {
        let mut pending: VecDeque<_> = root_ids.iter().map(|index| (*index, 0)).collect();
        let mut visited = BTreeSet::new();
        let mut dependencies = Vec::new();

        while let Some((index, depth)) = pending.pop_front() {
            // Hashmaps and GasLimitsPrices are recursive. Each declaration is
            // included once, while every constructor of a referenced type is kept.
            if !visited.insert(index) {
                continue;
            }

            // Show the types named in the parameter before their implementation
            // details, so a dictionary's value type is not buried below Hashmap.
            if !root_ids.contains(&index) {
                let result = declarations[index]
                    .combinator()
                    .context("missing dependency result type")?;
                dependencies.push(Dependency {
                    declaration: index,
                    depth,
                    parameterized: result.params().next().is_some(),
                    type_name: result
                        .name()
                        .context("missing dependency result type name")?
                        .text(source)
                        .to_owned(),
                });
            }

            for reference in &references[index] {
                if let Some(targets) = types.get(reference) {
                    ensure!(
                        *reference != "ConfigParam",
                        "ConfigParam {id} references ConfigParam recursively"
                    );
                    pending.extend(targets.iter().map(|target| (*target, depth + 1)));
                }
            }
        }

        used.extend(&visited);
        parameters.insert(
            id,
            ParameterSchema {
                line: declarations[root_ids[0]].syntax().start_position().row + 1,
                roots: root_ids,
                dependencies,
            },
        );
    }

    Ok(Catalog {
        parameters,
        declarations: used
            .into_iter()
            .map(|index| (index, format_declaration(declarations[index], source)))
            .collect(),
    })
}

/// Breaks multi-field constructors into readable lines without reprinting field
/// expressions. Comments stay in their original position; a trailing line
/// comment must never swallow the result type appended by the formatter.
fn format_declaration(declaration: Declaration<'_>, source: &str) -> String {
    if declaration.fields().count() <= 1 {
        return declaration.text(source).to_owned();
    }

    let mut output = String::new();
    let mut last_was_comment = false;
    let mut cursor = declaration.syntax().walk();

    for node in declaration.syntax().named_children(&mut cursor) {
        let text = &source[node.byte_range()];
        match node.kind() {
            "constructor_" => output.push_str(text),
            "field" | "comment" => {
                output.push_str("\n  ");
                output.push_str(text);
                last_was_comment = node.kind() == "comment";
            }
            "combinator" => {
                output.push_str(if last_was_comment { "\n  = " } else { " = " });
                output.push_str(text);
                output.push(';');
            }
            _ => {}
        }
    }

    output
}

/// Collects type references from fields, excluding implicit parameters and local
/// field names used in sizes/constraints. Primitives have no declarations in the
/// source; the caller's TL-B codegen validates each resulting standalone schema.
fn referenced_types<'source>(
    declaration: Declaration<'_>,
    source: &'source str,
) -> BTreeSet<&'source str> {
    let mut references = BTreeSet::new();
    let mut locals = BTreeSet::new();

    for field in declaration.fields() {
        for node in PreorderTraverse::new(field.syntax().walk()) {
            if matches!(
                node.kind(),
                "field_builtin" | "field_named" | "field_named_anon_ref"
            ) && let Some(name) = node.child_by_field_name("name")
            {
                locals.insert(&source[name.byte_range()]);
            }

            if node.kind() == "type_identifier" {
                references.insert(&source[node.byte_range()]);
            }
        }
    }

    references.retain(|name| !locals.contains(name));
    references
}

#[cfg(test)]
mod tests {
    use super::generate_catalog;
    use expect_test::expect;

    #[test]
    fn preserves_shared_recursive_and_parameterized_schemas() -> anyhow::Result<()> {
        let source = r"unused$_ = Unused;
x$_ = X;
none$0 {X:Type} = Maybe X;
some$1 {X:Type} value:X = Maybe X;
end$0 amount:uint64 = Price;
next$1 rest:^Price = Price;
_ {n:#} len:(#< n) value:(uint (len * 8)) = VarUInteger n;
grams$_ amount:(VarUInteger 16) = Grams;
_ amount:Grams price:(Maybe Price) = ConfigParam 17;
_ Price = ConfigParam 20;
_ alternate:^Price = ConfigParam 20;
commented#12 count:# // Keep the width explanation
  data:(bits count) { count <= 256 } = ConfigParam 21;
";
        let catalog = generate_catalog(source)?;
        expect![[r#"
            {
              "parameters": {
                "17": {
                  "line": 9,
                  "roots": [
                    8
                  ],
                  "dependencies": [
                    {
                      "declaration": 7,
                      "depth": 1,
                      "parameterized": false,
                      "typeName": "Grams"
                    },
                    {
                      "declaration": 2,
                      "depth": 1,
                      "parameterized": true,
                      "typeName": "Maybe"
                    },
                    {
                      "declaration": 3,
                      "depth": 1,
                      "parameterized": true,
                      "typeName": "Maybe"
                    },
                    {
                      "declaration": 4,
                      "depth": 1,
                      "parameterized": false,
                      "typeName": "Price"
                    },
                    {
                      "declaration": 5,
                      "depth": 1,
                      "parameterized": false,
                      "typeName": "Price"
                    },
                    {
                      "declaration": 6,
                      "depth": 2,
                      "parameterized": true,
                      "typeName": "VarUInteger"
                    }
                  ]
                },
                "20": {
                  "line": 10,
                  "roots": [
                    9,
                    10
                  ],
                  "dependencies": [
                    {
                      "declaration": 4,
                      "depth": 1,
                      "parameterized": false,
                      "typeName": "Price"
                    },
                    {
                      "declaration": 5,
                      "depth": 1,
                      "parameterized": false,
                      "typeName": "Price"
                    }
                  ]
                },
                "21": {
                  "line": 12,
                  "roots": [
                    11
                  ],
                  "dependencies": []
                }
              },
              "declarations": {
                "2": "none$0 {X:Type} = Maybe X;",
                "3": "some$1\n  {X:Type}\n  value:X = Maybe X;",
                "4": "end$0 amount:uint64 = Price;",
                "5": "next$1 rest:^Price = Price;",
                "6": "_\n  {n:#}\n  len:(#< n)\n  value:(uint (len * 8)) = VarUInteger n;",
                "7": "grams$_ amount:(VarUInteger 16) = Grams;",
                "8": "_\n  amount:Grams\n  price:(Maybe Price) = ConfigParam 17;",
                "9": "_ Price = ConfigParam 20;",
                "10": "_ alternate:^Price = ConfigParam 20;",
                "11": "commented#12\n  count:#\n  // Keep the width explanation\n  data:(bits count)\n  { count <= 256 } = ConfigParam 21;"
              }
            }"#]]
        .assert_eq(&serde_json::to_string_pretty(&catalog)?);
        Ok(())
    }

    #[test]
    fn rejects_recovered_syntax_and_nonliteral_parameter_ids() {
        let errors = [
            "_ broken:uint32 = ConfigParam 17",
            "_ {n:#} value:uint32 = ConfigParam n;",
            "_ value:uint32 = Other;",
        ]
        .map(|source| generate_catalog(source).unwrap_err().to_string());

        expect![[r#"
            [
                "TL-B source contains syntax errors",
                "ConfigParam ID must be a nonnegative integer literal",
                "TL-B source contains no ConfigParam declarations",
            ]
        "#]]
        .assert_debug_eq(&errors);
    }
}
