//! Compile debugger calls against the selected source file and run them in isolation.

use super::{
    ParsedValuePath, PathSegment, normalize_identifier, parse_expr_to_path, parse_wrapped_source,
    wrapped_expression,
};
use crate::core::replayer::LocalVarRuntime;
use crate::types_render::{RenderedValue, exact_slice_cell, render_tuple_as_tolk_type};
use anyhow::{Context, Result, anyhow, bail};
use std::collections::BTreeMap;
use std::fs;
use std::ops::Range;
use std::path::Path;
use tolk_compiler::Compiler;
use tolk_source_map::source_map::SourceMap;
use tolk_source_map::types_kernel::{Ty, TyIdx, calc_width_on_stack, render_ty};
use tolk_syntax::{Call, DotAccess, Expr, HasName, Ident, InstanceArg, Lambda, Walker};
use ton_executor::get::{GetExecutor, GetMethodResult, RunGetMethodArgs};
use tvm_ffi::serde::{parse_tuple_item, serialize_tuple};
use tvm_ffi::stack::{Tuple, TupleItem};
use tvm_logs::parser::{CellLike, VmStackValue};
use tycho_types::boc::Boc;

const EVALUATE_HELPER_ENTRY: &str = "__acton_debug_eval_entry";

/// Environment used to evaluate calls in a separate VM without changing the debuggee.
///
/// The session owner supplies the initial execution arguments and import mappings.
/// Each evaluation restores c4, c5, and c7 from the stopped VM before executing
/// the expression. Mutations affect only the isolated VM, not the debuggee.
/// Global slots in c7 are copied unchanged, so their compiled layout must remain stable.
#[derive(Debug, Clone)]
pub struct EvaluateRuntimeConfig {
    /// Initial contract state and execution environment; helper code replaces `code`.
    pub run_args: RunGetMethodArgs,
    /// Blockchain configuration for the isolated VM, encoded as a `BoC`.
    pub config_b64: Option<String>,
    /// Import mappings from the project that owns the selected source file.
    pub mappings: Option<BTreeMap<String, String>>,
}

pub(crate) fn is_function_call_expression(expression: &str) -> Result<bool> {
    let source_file = parse_wrapped_source(expression)?;
    let mut expr = wrapped_expression(&source_file)
        .ok_or_else(|| anyhow!("expected a single expression statement"))?;
    while let Expr::Paren(paren) = expr {
        expr = paren
            .inner()
            .ok_or_else(|| anyhow!("expected expression inside parentheses"))?;
    }

    Ok(matches!(expr, Expr::Call(_)))
}

pub(crate) fn evaluate_function_call(
    source_path: &Path,
    source_map: &SourceMap,
    runtime: &EvaluateRuntimeConfig,
    registers: [String; 3],
    runtime_locals: &[LocalVarRuntime],
    expression: &str,
) -> Result<RenderedValue> {
    let mut helper_call = plan_runtime_helper_call(source_map, runtime_locals, expression)?;
    let helper_code =
        build_runtime_arg_helper_code(&helper_call.expression, &helper_call.parameters);
    let original_source = fs::read_to_string(source_path)
        .with_context(|| format!("Cannot read source file {}", source_path.display()))?;
    let helper_source = format!("{original_source}{helper_code}");

    // Tolk registers stdlib as file 0 and the compilation entrypoint as file 1.
    // Keep the original import graph while placing the helper in the selected frame's scope.
    let entrypoint = Path::new(source_map.resolve_file_full_path(1).ok_or_else(|| {
        anyhow!("Cannot resolve the original compilation entrypoint for debugger evaluate")
    })?);

    let compiler = Compiler::new(2)
        .with_mappings(&runtime.mappings)
        .with_allow_no_entrypoint(true)
        .with_source_overrides([(source_path, helper_source)]);

    let compiled = match compiler.compile(entrypoint, true) {
        tolk_compiler::CompilerResult::Success(result) => result,
        tolk_compiler::CompilerResult::Error(error) => {
            bail!(
                "Debugger evaluate failed to compile `{expression}`:\n{}",
                error.message.trim()
            )
        }
    };

    let registers = registers
        .iter()
        .zip([4, 5, 7])
        .map(|(boc, index)| {
            let cell = Boc::decode_base64(boc)
                .with_context(|| format!("Cannot decode current c{index} for debugger evaluate"))?;
            parse_tuple_item(&mut cell.as_slice_allow_exotic()).with_context(|| {
                format!("Cannot deserialize current c{index} for debugger evaluate")
            })
        })
        .collect::<Result<Vec<_>>>()?;

    helper_call.stack.0.extend(registers);

    let method_id = compiled
        .abi
        .as_ref()
        .and_then(|abi| {
            abi.get_methods
                .iter()
                .find(|method| method.name == EVALUATE_HELPER_ENTRY)
                .map(|method| method.tvm_method_id)
        })
        .ok_or_else(|| anyhow!("Debugger evaluate failed to resolve compiled helper method id"))?;

    let mut params = runtime.run_args.clone();
    params.code.clone_from(&compiled.code_boc64);
    params.method_id = method_id;
    params.debug_enabled = true;

    let stack_b64 = Boc::encode_base64(serialize_tuple(&helper_call.stack)?);
    let executor = GetExecutor::new(&params)
        .with_context(|| format!("Debugger evaluate failed to prepare `{expression}`"))?;
    let result = executor
        .run_get_method(&stack_b64, &params, runtime.config_b64.as_deref())
        .with_context(|| format!("Debugger evaluate failed to execute `{expression}`"))?;

    let helper_source_map = compiled
        .source_map
        .as_ref()
        .context("Debugger evaluate helper has no source map")?;
    let return_ty_idx = helper_source_map
        .get_function_by_name(EVALUATE_HELPER_ENTRY)
        .context("Debugger evaluate helper has no return type")?
        .return_ty_idx;

    match result {
        GetMethodResult::Success(success)
            if success.vm_exit_code == 0 || success.vm_exit_code == 1 =>
        {
            let stack_cell = Boc::decode_base64(success.stack.as_ref())
                .context("Failed to decode evaluate result stack")?;
            let stack_tuple = Tuple::deserialize(&stack_cell)
                .context("Failed to deserialize evaluate result stack")?;

            // ABI client types describe serialized data and can differ from the VM stack layout.
            Ok(render_tuple_as_tolk_type(
                helper_source_map,
                &stack_tuple,
                return_ty_idx,
            ))
        }
        GetMethodResult::Success(success) => bail!(
            "Debugger evaluate failed for `{expression}` with VM exit code {}.\nVM log:\n{}",
            success.vm_exit_code,
            success.vm_log,
        ),
        GetMethodResult::Error(error) => {
            bail!(
                "Debugger evaluate failed for `{expression}`: {}",
                error.error
            )
        }
    }
}

/// Source ranges identify value uses without rewriting method names, types, or string literals.
struct RuntimeLocalUse {
    path: ParsedValuePath,
    range: Range<usize>,
    shorthand_field: bool,
}

struct RuntimeLocalUses<'a> {
    source: &'a str,
    locals: &'a [LocalVarRuntime],
    uses: Vec<RuntimeLocalUse>,
    has_lambda: bool,
}

impl RuntimeLocalUses<'_> {
    fn capture_path(&mut self, expr: Expr<'_>, shorthand_field: bool) -> bool {
        let Ok(path) = parse_expr_to_path(expr, self.source) else {
            return false;
        };
        if !self
            .locals
            .iter()
            .any(|local| normalize_identifier(&local.var_name) == path.root)
        {
            return false;
        }
        self.uses.push(RuntimeLocalUse {
            path,
            range: expr.syntax().byte_range(),
            shorthand_field,
        });
        true
    }
}

impl<'tree> Walker<'tree> for RuntimeLocalUses<'_> {
    type Result = ();

    fn default_result(&self) {}

    fn walk_ident(&mut self, ident: &Ident<'tree>) {
        self.capture_path(Expr::Ident(*ident), false);
    }

    fn walk_dot_access(&mut self, access: &DotAccess<'tree>) {
        if !self.capture_path(Expr::DotAccess(*access), false)
            && let Some(object) = access.obj()
        {
            self.visit_expr(&object);
        }
    }

    fn walk_call(&mut self, call: &Call<'tree>) {
        // The qualifier is a value; the method name after the dot is not a local variable.
        if let Some(receiver) = call.callee_qualifier() {
            self.visit_expr(&receiver);
        } else if let Some(callee) = call.callee() {
            self.visit_expr(&callee);
        }
        for argument in call.arguments() {
            self.walk_call_argument(&argument);
        }
    }

    fn walk_instance_arg(&mut self, argument: &InstanceArg<'tree>) {
        if argument.has_value_separator() {
            if let Some(value) = argument.value() {
                self.visit_expr(&value);
            }
        } else if let Some(name) = argument.name() {
            self.capture_path(Expr::Ident(name), true);
        }
    }

    fn walk_lambda(&mut self, _lambda: &Lambda<'tree>) {
        // Lambda parameters introduce a separate scope, so outer locals cannot be substituted by name.
        self.has_lambda = true;
    }
}

#[derive(Debug, Clone)]
struct HelperRuntimeParameter {
    name: String,
    type_name: String,
    stack_items: Vec<TupleItem>,
}

#[derive(Debug, Clone)]
struct PlannedRuntimeHelperCall {
    parameters: Vec<HelperRuntimeParameter>,
    expression: String,
    stack: Tuple,
}

/// Bind observed values once, then let Tolk compile the original calls and operators.
/// Reusing a parameter for repeated paths preserves mutation order within this isolated expression.
fn plan_runtime_helper_call(
    source_map: &SourceMap,
    runtime_locals: &[LocalVarRuntime],
    expression: &str,
) -> Result<PlannedRuntimeHelperCall> {
    let source_file = parse_wrapped_source(expression)?;
    let expr = wrapped_expression(&source_file)
        .ok_or_else(|| anyhow!("expected a single expression statement"))?;
    let source = source_file.source.as_ref();
    let mut uses = RuntimeLocalUses {
        source,
        locals: runtime_locals,
        uses: Vec::new(),
        has_lambda: false,
    };
    uses.visit_expr(&expr);
    if uses.has_lambda {
        bail!("Lambda expressions are not supported in debugger evaluate yet");
    }

    let mut parameters = Vec::<HelperRuntimeParameter>::new();
    let mut parameter_paths = Vec::<ParsedValuePath>::new();
    let mut replacements = Vec::new();
    for usage in uses.uses {
        let index = if let Some(index) = parameter_paths.iter().position(|path| *path == usage.path)
        {
            index
        } else {
            let local = runtime_locals
                .iter()
                .rev()
                .find(|local| normalize_identifier(&local.var_name) == usage.path.root)
                .ok_or_else(|| anyhow!("Variable `{}` is not in scope", usage.path.root))?;
            let index = parameters.len();
            parameters.push(build_runtime_parameter_for_path(
                source_map,
                local,
                &usage.path,
                helper_runtime_param_name(index),
            )?);
            parameter_paths.push(usage.path);
            index
        };
        let parameter = &parameters[index].name;
        let replacement = if usage.shorthand_field {
            format!("{}: {parameter}", &source[usage.range.clone()])
        } else {
            parameter.clone()
        };
        replacements.push((usage.range, replacement));
    }

    let start = expr.syntax().start_byte();
    let mut expression = expr.text(source).to_owned();
    // Apply edits from the end so earlier byte offsets remain valid, including non-ASCII identifiers.
    replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    for (range, replacement) in replacements {
        expression.replace_range(range.start - start..range.end - start, &replacement);
    }
    let stack = Tuple(
        parameters
            .iter()
            .flat_map(|parameter| parameter.stack_items.iter().cloned())
            .collect(),
    );
    Ok(PlannedRuntimeHelperCall {
        parameters,
        expression,
        stack,
    })
}

fn build_runtime_parameter_for_path(
    source_map: &SourceMap,
    local: &LocalVarRuntime,
    path: &ParsedValuePath,
    parameter_name: String,
) -> Result<HelperRuntimeParameter> {
    let ty = local.ty_idx.ok_or_else(|| {
        anyhow!(
            "Variable `{}` has no source-level type available for debugger evaluate",
            path.root
        )
    })?;
    let (resolved_ty, resolved_slot_values) =
        resolve_runtime_path_values(source_map, ty, &local.ir_slot_values, &path.segments)
            .with_context(|| {
                if path.segments.is_empty() {
                    format!(
                        "Variable `{}` is not available on the runtime stack",
                        path.root
                    )
                } else {
                    format!(
                        "Path `{}` is not available on the runtime stack",
                        render_value_path(path)
                    )
                }
            })?;
    let stack_items = resolved_slot_values
        .into_iter()
        .map(convert_vm_value_to_tuple_item)
        .collect::<Result<Vec<_>>>()?;

    Ok(HelperRuntimeParameter {
        name: parameter_name,
        type_name: render_ty(source_map, resolved_ty),
        stack_items,
    })
}

/// Resolve only observed slots; unrelated unloaded lazy fields may remain unavailable.
fn resolve_runtime_path_values(
    source_map: &SourceMap,
    ty_idx: TyIdx,
    slots: &[Option<VmStackValue>],
    segments: &[PathSegment],
) -> Result<(TyIdx, Vec<VmStackValue>)> {
    let Some((segment, remaining)) = segments.split_first() else {
        let values = slots
            .iter()
            .cloned()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| anyhow!("required runtime slots were not observed"))?;
        return Ok((ty_idx, values));
    };
    let ty = source_map
        .ty_by_idx(ty_idx)
        .ok_or_else(|| anyhow!("source-level type is unavailable"))?;
    let (next_ty, offset) = match (ty, segment) {
        (Ty::AliasRef { .. }, _) => {
            let target = source_map
                .alias_target_of(ty_idx)
                .ok_or_else(|| anyhow!("alias target is unavailable"))?;
            return resolve_runtime_path_values(source_map, target, slots, segments);
        }
        (Ty::StructRef { .. }, PathSegment::Field(name)) => {
            let fields = source_map
                .struct_fields_of(ty_idx)
                .ok_or_else(|| anyhow!("struct fields are unavailable"))?;
            let mut offset = 0;
            let field = fields
                .iter()
                .find(|field| {
                    if field.name == *name {
                        return true;
                    }
                    offset += calc_width_on_stack(source_map, field.ty_idx);
                    false
                })
                .ok_or_else(|| {
                    anyhow!(
                        "Field `{name}` is not available on `{}`",
                        render_ty(source_map, ty_idx)
                    )
                })?;
            (field.ty_idx, offset)
        }
        (Ty::Tensor { items_ty_idx }, PathSegment::Index(index)) => {
            let next_ty = *items_ty_idx
                .get(*index)
                .ok_or_else(|| anyhow!("Index {index} is out of bounds"))?;
            let offset = items_ty_idx
                .iter()
                .take(*index)
                .map(|idx| calc_width_on_stack(source_map, *idx))
                .sum();
            (next_ty, offset)
        }
        (Ty::ShapedTuple { items_ty_idx }, PathSegment::Index(index)) => {
            let next_ty = *items_ty_idx
                .get(*index)
                .ok_or_else(|| anyhow!("Index {index} is out of bounds"))?;
            let Some(Some(VmStackValue::Tuple(items))) = slots.first() else {
                bail!("tuple value was not observed");
            };
            let value = items
                .get(*index)
                .ok_or_else(|| anyhow!("runtime tuple index is out of bounds"))?;
            let values = match value {
                VmStackValue::Tuple(values) if calc_width_on_stack(source_map, next_ty) != 1 => {
                    values.iter().cloned().map(Some).collect::<Vec<_>>()
                }
                value => vec![Some(value.clone())],
            };
            return resolve_runtime_path_values(source_map, next_ty, &values, remaining);
        }
        _ => bail!(
            "Cannot access {segment:?} on `{}`",
            render_ty(source_map, ty_idx)
        ),
    };
    let width = calc_width_on_stack(source_map, next_ty);
    let values = slots
        .get(offset..offset + width)
        .ok_or_else(|| anyhow!("runtime slot range is out of bounds"))?;
    resolve_runtime_path_values(source_map, next_ty, values, remaining)
}

fn render_value_path(path: &ParsedValuePath) -> String {
    let mut rendered = path.root.clone();
    for segment in &path.segments {
        rendered.push('.');
        match segment {
            PathSegment::Field(field_name) => rendered.push_str(field_name),
            PathSegment::Index(index) => rendered.push_str(&index.to_string()),
        }
    }
    rendered
}

fn helper_runtime_param_name(index: usize) -> String {
    format!("__acton_debug_eval_arg{index}")
}

fn convert_vm_value_to_tuple_item(value: VmStackValue) -> Result<TupleItem> {
    match value {
        VmStackValue::Null => Ok(TupleItem::Null),
        VmStackValue::NaN => Ok(TupleItem::Nan),
        VmStackValue::Integer(value) => {
            let value = value
                .parse()
                .map_err(|_| anyhow!("Invalid integer stack value: {value}"))?;
            Ok(TupleItem::Int(value))
        }
        VmStackValue::Tuple(values) => Ok(TupleItem::Tuple(Tuple(
            values
                .into_iter()
                .map(convert_vm_value_to_tuple_item)
                .collect::<Result<Vec<_>>>()?,
        ))),
        VmStackValue::Cell(CellLike::Cell(hex) | CellLike::Builder(hex)) => {
            Ok(TupleItem::Cell(Boc::decode_hex(hex)?))
        }
        VmStackValue::Builder(hex) => Ok(TupleItem::Builder(Boc::decode_hex(hex)?)),
        VmStackValue::CellSlice(slice) => Ok(TupleItem::Slice(
            exact_slice_cell(&slice).ok_or_else(|| anyhow!("Cannot decode runtime slice"))?,
        )),
        VmStackValue::Continuation(_) => {
            bail!("Continuation values are not supported in debugger evaluate arguments yet")
        }
        VmStackValue::String(value) => {
            let mut tuple = Tuple::empty();
            tuple.push_string(&value);
            tuple
                .0
                .pop()
                .ok_or_else(|| anyhow!("Cannot encode string argument"))
        }
        VmStackValue::Unknown => bail!("Unknown runtime stack values are not supported"),
    }
}

fn build_runtime_arg_helper_code(
    expression: &str,
    parameters: &[HelperRuntimeParameter],
) -> String {
    let mut helper_args = parameters
        .iter()
        .map(|parameter| format!("{}: {}", parameter.name, parameter.type_name))
        .collect::<Vec<_>>()
        .join(", ");
    if !helper_args.is_empty() {
        helper_args.push_str(", ");
    }
    helper_args.push_str("__c4: cell, __c5: cell, __c7: tuple");
    format!(
        "

fun __acton_debug_eval_restore(__c4: cell, __c5: cell, __c7: tuple): void
    asm \"c7 POPCTR\" \"c5 POPCTR\" \"c4 POPCTR\"

get fun {EVALUATE_HELPER_ENTRY}({helper_args}) {{
    __acton_debug_eval_restore(__c4, __c5, __c7);
    return {expression};
}}
"
    )
}
