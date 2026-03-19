// TolkReplayer — walks through TVM execution log step by step,
// applying debug marks to reconstruct source-level state:
// which function we're in, what variables are on the stack,
// what source line corresponds to the current instruction.

use crate::types_render::{RenderedValue, SlotValue, debug_format_lazy, debug_print_from_stack};
use std::collections::{HashMap, HashSet, VecDeque};
use tolkc::debug_marks_dict::DebugMarksDict;
use tolkc::source_map::{DebugMark, SourceMap, SrcRange};
use tolkc::types_kernel::Ty;
use vmlogs::parser::{VmLine, VmStackValue};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepMode {
    EachAsmInstruction,
    StepOver,
    StepInto,
    StepOut,
    RunUntilBreakpoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionBreakMode {
    Never,
    Uncaught,
    All,
}

#[derive(Debug, Clone)]
pub struct ExceptionInfo {
    pub errno: String,
    pub is_uncaught: bool,
}

#[derive(Debug, Clone)]
pub struct CallFrameInfo {
    pub f_name: String,
    pub is_inlined: bool,
    pub is_builtin: bool,
    pub call_site_loc: Option<SrcRange>,
}

#[derive(Debug, Clone)]
pub struct LocalVarRendered {
    pub var_name: String,
    pub value: RenderedValue,
}

#[derive(Debug, Clone)]
struct LocalVarInScope {
    name: String,
    ty_idx: usize,
    ir_slots: Vec<usize>,
    is_lazy: bool,
}

#[derive(Debug, Clone)]
struct LexicalScope {
    range: SrcRange,
    variables: Vec<LocalVarInScope>,
}

/// Per-noinline-function execution state. Each noinline call creates its own
/// IR slot namespace, so ir_stack / last_seen must be tracked independently.
/// Pushed onto `exec_stack` when entering a noinline function.
#[derive(Debug, Clone)]
struct NoinlineExecState {
    // current IR slot layout of the TVM stack (from last MARK_STACK)
    ir_stack: Vec<usize>,
    // number of TVM stack entries below IR slots (continuations, etc.)
    system_stack_depth: usize,
    // last known TVM value for each IR slot, for showing "last seen" variables
    last_seen_values: HashMap<usize, VmStackValue>,
    // union of all IR slots seen from MARK_STACK ticks since the last TVM instruction;
    // between TVM instructions, the physical stack doesn't change, but IR slot names
    // may be renamed (e.g. `var aa = v; var bb = aa;`), accumulating all of them lets
    // format_locals show every variable as "live" rather than "last seen"
    accumulated_ir_live: HashSet<usize>,
    // set by TvmStackValues; the next StackLayout on this context will replace
    // accumulated_ir_live instead of extending it;
    // we can't just clear() instead of setting this to true, because of how CALLDICT
    // (noinline functions) works: it updates TVM stack in caller's before jumping
    accumulated_needs_reset: bool,
}

impl NoinlineExecState {
    fn new() -> Self {
        NoinlineExecState {
            ir_stack: Vec::new(),
            system_stack_depth: 0,
            last_seen_values: HashMap::new(),
            accumulated_ir_live: HashSet::new(),
            accumulated_needs_reset: false,
        }
    }
}

#[derive(Debug, Clone)]
struct CallFrame {
    // index in source_map.functions
    f_idx: usize,
    // equals to source_map.functions[f_idx].name, stored for easier debugging
    f_name: String,
    is_inlined: bool,
    is_builtin: bool,
    call_site_loc: Option<SrcRange>,
    variables: Vec<LocalVarInScope>,
    scope_stack: Vec<LexicalScope>,
    // set when LEAVE_FUN is processed, before the frame is popped;
    // used by format_locals to show "(return value)"
    pending_ir_return: Option<Vec<usize>>,
}

impl CallFrame {
    /// Returns the variable list where new variables should be added:
    /// the innermost open scope, or the frame's own top-level list.
    fn current_vars_mut(&mut self) -> &mut Vec<LocalVarInScope> {
        if let Some(scope) = self.scope_stack.last_mut() {
            &mut scope.variables
        } else {
            &mut self.variables
        }
    }

    /// Iterate over all visible variables: frame-level + all open scopes, in order.
    fn all_visible_vars(&self) -> impl Iterator<Item = &LocalVarInScope> {
        self.variables
            .iter()
            .chain(self.scope_stack.iter().flat_map(|s| s.variables.iter()))
    }
}

/// Pre-converted VM log line with all owned data.
/// Created once from `parser::VmLine<'a>` to eliminate lifetimes.
enum OwnedVmLine {
    Stack { tvm_stack_values: Vec<VmStackValue> },
    Loc { cell_hash: String, offset: i32 },
    Execute { instr_name: String },
    Exception { errno: String },
    ExceptionHandler { errno: String },
}

fn convert_vm_lines(parsed: &[Result<VmLine<'_>, String>]) -> Vec<OwnedVmLine> {
    parsed
        .iter()
        .filter_map(|r| match r {
            Ok(VmLine::VmStack { stack }) => Some(OwnedVmLine::Stack {
                tvm_stack_values: stack.parsed(),
            }),
            Ok(VmLine::VmLoc { hash, offset }) => Some(OwnedVmLine::Loc {
                cell_hash: hash.to_string(),
                offset: offset.parse().unwrap_or(0),
            }),
            Ok(VmLine::VmExecute { instr }) => Some(OwnedVmLine::Execute {
                instr_name: instr.to_string(),
            }),
            Ok(VmLine::VmException { errno, .. }) => Some(OwnedVmLine::Exception {
                errno: errno.to_string(),
            }),
            Ok(VmLine::VmExceptionHandler { errno }) => Some(OwnedVmLine::ExceptionHandler {
                errno: errno.to_string(),
            }),
            // we don't need other lines from TVM execution logs (about gas limits, c5, etc.)
            _ => None,
        })
        .collect()
}

// (cell_hash, offset) -> sorted vec of mark_id into source_map.debug_marks
type MarksLookup = HashMap<(String, i32), Vec<usize>>;

/// Tick — atomic unit of work for the replayer.
///
/// The tick stream is lazily built from VM log lines and debug marks.
/// Stored as Replayer::pending_ticks (the "current position" of the replayer).
/// Returned by step_verbose() for logging/monitoring.
#[derive(Debug, Clone)]
pub enum Tick {
    Loc {
        range: SrcRange,
    },
    AtFunReturn {
        f_idx: usize,
        range: SrcRange,
        ir_return: Vec<usize>,
    },

    PushFrame {
        f_idx: usize,
        is_inlined: bool,
        is_builtin: bool,
        call_site_range: SrcRange,
        ir_import: Vec<usize>,
    },
    PopFrame {
        f_idx: usize,
    },
    StackLayout {
        ir_stack: Vec<usize>,
    },
    LocalVar {
        var_name: String,
        ty_idx: usize,
        ir_slots: Vec<usize>,
        is_parameter: bool,
        is_lazy: bool,
    },
    SmartCast {
        var_name: String,
        ty_idx: usize,
        ir_slots: Vec<usize>,
    },
    SetGlob {
        glob_name: String,
        ty_idx: usize,
        ir_slots: Vec<usize>,
    },
    ScopeStart {
        range: SrcRange,
    },
    ScopeEnd,

    TvmStackValues {
        values: Vec<VmStackValue>,
    },
    TvmBeforeExecute,
    TvmAfterExecute {
        instr_name: String,
    },
    TvmImplicitJmpRef,
    TvmException {
        errno: String,
    },
    TvmExceptionHandler {
        errno: String,
    },
}

// ---------------------------------------------------------------------------
// TolkReplayer
// ---------------------------------------------------------------------------

pub struct TolkReplayer {
    // parsed source map JSON: files, functions, types, debug marks, declarations
    source_map: SourceMap,
    // (cell_hash, offset) → mark_id mapping built from Fift debug marks dictionary
    marks_lookup: MarksLookup,

    // TVM execution log (cell hashes and offsets, stack values, executed asm instructions)
    vm_lines: Vec<OwnedVmLine>,
    // cursor inside TVM execution log, it's parsed top-down, "replaying" execution trace
    cur_vm_line_idx: usize,

    // source location where execution last stopped (file, line, column)
    current_loc: SrcRange,

    // source-level call stack: one entry per function (including inlined and built-in)
    call_stack: Vec<CallFrame>,
    // per-noinline-context state; pushed/popped in sync with noinline call frames;
    // exec_stack[0] is the root context, exec_stack[1] = "main" appears after DICTIGETJMPZ
    exec_stack: Vec<NoinlineExecState>,

    // glob_name → (ty_idx, captured TVM values) for globals that have been SET
    global_var_values: HashMap<String, (usize, Vec<VmStackValue>)>,

    // raw TVM stack (updated from STACK lines in the VM log);
    // global (not per-context) because TvmStackValues tick arrives before PushFrame
    tvm_stack_values: Vec<VmStackValue>,

    // active breakpoints as (file_id, line) pairs
    breakpoints: HashSet<(usize, usize)>,

    // when to pause on exceptions: Never / Uncaught / All
    exception_break_mode: ExceptionBreakMode,
    // set when an exception is thrown; cleared on next normal tick or step()
    last_exception: Option<ExceptionInfo>,

    // after PUSHCONT, inline continuation data shares offsets with the main flow,
    // so marks at those offsets belong to the continuation body, not the main flow;
    // suppress mark decoding until the control-flow instruction (IFELSE etc.) executes
    prev_was_pushcont: bool,

    // TVM sometimes jumps to a location outside the current function
    // without passing through MARK_LEAVE_FUN: exceptions caught by TRY,
    // IFRET/IFNOTRET (Fift optimizes IFJMP:<{empty}> → IFRET), etc.
    after_exception_ifret: bool,

    // line and call depth at the last stop; used by should_stop() for step over/into/out
    last_stop_line: usize,
    last_stop_depth: usize,
    // when resuming, skip breakpoints on this line until we visit a different line;
    // prevents re-triggering on the same LOC when multiple marks share a line
    breakpoint_skip_line: usize,

    // tick queue: expand_mark_to_ticks() may produce several ticks from one debug mark
    pending_ticks: VecDeque<Tick>,
}

impl TolkReplayer {
    pub fn new(
        source_map: SourceMap,
        marks_dict: &DebugMarksDict,
        vm_lines: &[Result<VmLine<'_>, String>],
    ) -> Self {
        let mut lookup = MarksLookup::new();
        for (cell_hash, entries) in marks_dict {
            for &(offset, mark_id) in entries {
                lookup
                    .entry((cell_hash.clone(), offset))
                    .or_default()
                    .push(mark_id as usize); // mark_id is 0-indexed
            }
        }

        TolkReplayer {
            source_map,
            marks_lookup: lookup,
            vm_lines: convert_vm_lines(vm_lines),
            cur_vm_line_idx: 0,
            call_stack: Vec::new(),
            current_loc: SrcRange(vec![0, 0, 0, 0, 0]),
            exec_stack: vec![NoinlineExecState::new()],
            global_var_values: HashMap::new(),
            tvm_stack_values: Vec::new(),
            breakpoints: HashSet::new(),
            exception_break_mode: ExceptionBreakMode::Never,
            last_exception: None,
            prev_was_pushcont: false,
            after_exception_ifret: false,
            last_stop_line: 0,
            last_stop_depth: usize::MAX,
            breakpoint_skip_line: 0,
            pending_ticks: VecDeque::new(),
        }
    }

    /// Set breakpoints for a file. Each requested line is resolved to the nearest
    /// line >= it that has a debug mark (LOC, inlined ENTER_FUN, or LEAVE_FUN),
    /// so breakpoints on optimized-away lines shift to the next stoppable line.
    pub fn set_breakpoints(&mut self, file_id: usize, lines: &[usize]) {
        let valid_lines = self.source_map.stoppable_lines_for_file(file_id);

        self.breakpoints.retain(|&(fid, _)| fid != file_id);
        for &line in lines {
            let resolved = valid_lines
                .iter()
                .find(|&&vl| vl >= line)
                .copied()
                .unwrap_or(line);
            self.breakpoints.insert((file_id, resolved));
        }
    }

    pub fn clear_all_breakpoints(&mut self) {
        self.breakpoints.clear();
    }

    pub const fn set_exception_breakpoints(&mut self, mode: ExceptionBreakMode) {
        self.exception_break_mode = mode;
    }

    pub const fn last_exception(&self) -> Option<&ExceptionInfo> {
        self.last_exception.as_ref()
    }

    pub fn is_finished(&self) -> bool {
        if self.last_exception.is_some() {
            return false;
        }
        self.cur_vm_line_idx >= self.vm_lines.len() && self.pending_ticks.is_empty()
    }

    pub fn current_file_id(&self) -> usize {
        self.current_loc.file_id()
    }

    pub fn current_file_name(&self) -> &str {
        self.file_display_name(self.current_loc.file_id())
    }

    pub fn current_line(&self) -> usize {
        self.current_loc.start_line()
    }

    pub fn current_column(&self) -> usize {
        self.current_loc.start_col()
    }

    pub fn function_name_by_idx(&self, f_idx: usize) -> String {
        self.source_map.get_function_name_by_idx(f_idx)
    }

    pub fn call_stack(&self) -> Vec<CallFrameInfo> {
        self.call_stack
            .iter()
            .map(|f| CallFrameInfo {
                f_name: f.f_name.clone(),
                is_inlined: f.is_inlined,
                is_builtin: f.is_builtin,
                call_site_loc: f.call_site_loc.clone(),
            })
            .collect()
    }

    /// Locals for a specific call frame. `depth` is 0 for the top (innermost) frame,
    /// 1 for its caller, etc.
    pub fn locals_for_frame(&self, depth: usize) -> Vec<LocalVarRendered> {
        let idx = self.call_stack.len().checked_sub(1 + depth);
        match idx {
            Some(i) => {
                let exec_idx = self.exec_idx_for_frame(i);
                let exec = &self.exec_stack[exec_idx];
                self.format_locals_of(
                    &self.call_stack[i],
                    &exec.last_seen_values,
                    &exec.accumulated_ir_live,
                )
            }
            None => Vec::new(),
        }
    }

    /// Map a call_stack frame index to the corresponding exec_stack index.
    /// call_stack also contains inlined/built-in functions, whereas exec_stack only noinline contexts.
    fn exec_idx_for_frame(&self, frame_idx: usize) -> usize {
        let mut idx = 0;
        for j in 1..=frame_idx {
            if !self.call_stack[j].is_inlined {
                idx += 1;
            }
        }
        idx // 0 (root, before entering "main" or get method) always exists
    }

    /// Full path for a file_id (as stored in source map JSON).
    pub fn file_full_path(&self, file_id: usize) -> Option<&str> {
        self.source_map.resolve_file_full_path(file_id)
    }

    /// Advance execution until the next stop (step/breakpoint) or end of log.
    pub fn step(&mut self, step_mode: StepMode) {
        self.last_exception = None;
        while let Some(tick) = self.next_tick() {
            if self.apply_tick(tick, step_mode) {
                self.record_stop();
                break;
            }
        }
    }

    /// Like step(), but calls `on_tick` after each tick is applied,
    /// giving the callback access to the replayer's up-to-date state.
    pub fn step_with_callback(
        &mut self,
        step_mode: StepMode,
        mut on_tick: impl FnMut(&Tick, &Self),
    ) {
        self.last_exception = None;
        while let Some(tick) = self.next_tick() {
            let should_stop = self.apply_tick(tick.clone(), step_mode);
            on_tick(&tick, self);
            if should_stop {
                self.record_stop();
                break;
            }
        }
    }

    /// Snapshot the current position as the last stop point.
    fn record_stop(&mut self) {
        self.last_stop_line = self.current_loc.start_line();
        self.last_stop_depth = self.call_stack.len();
        self.breakpoint_skip_line = self.last_stop_line;
    }

    /// Is triggered on location changed, save `current_loc`.
    fn assign_current_loc(&mut self, loc: &SrcRange) {
        self.current_loc = loc.clone();
        if loc.start_line() != self.breakpoint_skip_line {
            self.breakpoint_skip_line = 0;
        }
    }

    /// Formatted TVM stack (user-visible values, skipping system elements).
    pub fn tvm_stack_rendered(&self) -> Vec<String> {
        let exec = self
            .exec_stack
            .last()
            .expect("replayer invariant: exec_stack must contain the root execution state");
        let skip = exec.system_stack_depth.min(self.tvm_stack_values.len());
        self.tvm_stack_values[skip..]
            .iter()
            .map(|val| val.to_string())
            .collect()
    }

    /// Pull the next tick from the pending queue, or lazily expand VM log lines
    /// into ticks. Returns None when the VM log is exhausted.
    fn next_tick(&mut self) -> Option<Tick> {
        if let Some(tick) = self.pending_ticks.pop_front() {
            return Some(tick);
        }

        while self.cur_vm_line_idx < self.vm_lines.len() {
            let idx = self.cur_vm_line_idx;
            self.cur_vm_line_idx += 1;

            match &self.vm_lines[idx] {
                OwnedVmLine::Stack { tvm_stack_values } => {
                    return Some(Tick::TvmStackValues {
                        values: tvm_stack_values.clone(),
                    });
                }
                OwnedVmLine::Loc { cell_hash, offset } => {
                    if !self.prev_was_pushcont {
                        let key = (cell_hash.clone(), *offset);
                        if let Some(mark_indices) = self.marks_lookup.get(&key) {
                            let indices = mark_indices.clone();
                            for mark_id in indices {
                                if mark_id < self.source_map.debug_marks_count() {
                                    self.expand_mark_to_ticks(mark_id);
                                }
                            }
                            if let Some(tick) = self.pending_ticks.pop_front() {
                                return Some(tick);
                            }
                        }
                    }
                }
                OwnedVmLine::Execute { instr_name } => {
                    if instr_name == "implicit JMPREF" {
                        return Some(Tick::TvmImplicitJmpRef);
                    }
                    self.pending_ticks.push_back(Tick::TvmAfterExecute {
                        instr_name: instr_name.clone(),
                    });
                    return Some(Tick::TvmBeforeExecute);
                }
                OwnedVmLine::Exception { errno } => {
                    return Some(Tick::TvmException {
                        errno: errno.clone(),
                    });
                }
                OwnedVmLine::ExceptionHandler { errno } => {
                    return Some(Tick::TvmExceptionHandler {
                        errno: errno.clone(),
                    });
                }
            }
        }

        None
    }

    /// Convert a debug mark into one or more ticks appended to pending_ticks.
    /// ENTER_FUN and LEAVE_FUN are split: their range becomes a Tick::Loc,
    /// and the frame push/pop becomes a separate non-stoppable tick.
    fn expand_mark_to_ticks(&mut self, mark_id: usize) {
        let mark = self.source_map.get_debug_mark(mark_id);
        match mark {
            DebugMark::Loc { range, .. } => {
                self.pending_ticks.push_back(Tick::Loc {
                    range: range.clone(),
                });
            }
            DebugMark::Stack { stack, .. } => {
                self.pending_ticks.push_back(Tick::StackLayout {
                    ir_stack: stack.clone(),
                });
            }
            DebugMark::EnterFun {
                f_idx,
                is_inlined,
                is_builtin,
                range,
                ir_import,
                ..
            } => {
                if *is_inlined {
                    self.pending_ticks.push_back(Tick::Loc {
                        range: range.clone(),
                    });
                }
                self.pending_ticks.push_back(Tick::PushFrame {
                    f_idx: *f_idx,
                    is_inlined: *is_inlined,
                    is_builtin: *is_builtin,
                    call_site_range: range.clone(),
                    ir_import: ir_import.clone(),
                });
            }
            DebugMark::LeaveFun {
                f_idx,
                ir_return,
                range,
                ..
            } => {
                self.pending_ticks.push_back(Tick::AtFunReturn {
                    f_idx: *f_idx,
                    range: range.clone(),
                    ir_return: ir_return.clone(),
                });
                self.pending_ticks
                    .push_back(Tick::PopFrame { f_idx: *f_idx });
            }
            DebugMark::Var {
                var_name,
                ty_idx,
                ir_slots,
                is_parameter,
                is_lazy,
                ..
            } => {
                self.pending_ticks.push_back(Tick::LocalVar {
                    var_name: var_name.clone(),
                    ty_idx: *ty_idx,
                    ir_slots: ir_slots.clone(),
                    is_parameter: *is_parameter,
                    is_lazy: (*is_lazy).unwrap_or(false),
                });
            }
            DebugMark::SmartCast {
                var_name,
                ty_idx,
                ir_slots,
                ..
            } => {
                self.pending_ticks.push_back(Tick::SmartCast {
                    var_name: var_name.clone(),
                    ty_idx: *ty_idx,
                    ir_slots: ir_slots.clone(),
                });
            }
            DebugMark::SetGlob {
                glob_name,
                ty_idx,
                ir_slots,
                ..
            } => {
                self.pending_ticks.push_back(Tick::SetGlob {
                    glob_name: glob_name.clone(),
                    ty_idx: *ty_idx,
                    ir_slots: ir_slots.clone(),
                });
            }
            DebugMark::ScopeStart { range, .. } => {
                self.pending_ticks.push_back(Tick::ScopeStart {
                    range: range.clone(),
                });
            }
            DebugMark::ScopeEnd { .. } => {
                self.pending_ticks.push_back(Tick::ScopeEnd);
            }
        }
    }

    /// Process a single tick: mutate replayer state.
    /// Returns true if the debugger should stop after this tick.
    fn apply_tick(&mut self, tick: Tick, step_mode: StepMode) -> bool {
        match tick {
            Tick::Loc { range } => {
                self.clear_caught_exception();
                self.assign_current_loc(&range);
                if self.after_exception_ifret {
                    self.unwind_after_exception_ifret(&range);
                }
                return self.should_stop(step_mode, false);
            }
            Tick::AtFunReturn {
                f_idx,
                range,
                ir_return,
            } => {
                self.assign_current_loc(&range);
                if self.after_exception_ifret {
                    self.unwind_after_exception_ifret(&range);
                }
                let is_void = self
                    .source_map
                    .get_function_by_idx(f_idx)
                    .and_then(|f| self.source_map.resolve_ty(f.return_ty_idx))
                    .is_some_and(|ty| matches!(ty, Ty::Void));
                if let Some(frame) = self.call_stack.last_mut() {
                    frame.pending_ir_return = Some(ir_return);
                }
                if is_void {
                    // don't stop at closing brace `}` of void functions
                    return false; // (of non-void, we stop at 'return' statement and see "return value")
                }
                return self.should_stop(step_mode, true);
            }
            Tick::PushFrame {
                f_idx,
                is_inlined,
                is_builtin,
                call_site_range,
                ir_import,
            } => {
                if !is_inlined && !self.call_stack.is_empty() {
                    self.update_last_seen();
                    let system_depth = self.tvm_stack_values.len().saturating_sub(ir_import.len());
                    let mut last_seen = HashMap::new();
                    for (i, &ir_idx) in ir_import.iter().enumerate() {
                        if let Some(val) = self.tvm_stack_values.get(system_depth + i) {
                            last_seen.insert(ir_idx, val.clone());
                        }
                    }
                    self.exec_stack.push(NoinlineExecState {
                        ir_stack: ir_import,
                        system_stack_depth: system_depth,
                        last_seen_values: last_seen,
                        accumulated_ir_live: HashSet::new(),
                        accumulated_needs_reset: false,
                    });
                }
                // for inlined: call_site_range from MARK_ENTER_FUN is the call site;
                // for noinline: MARK_ENTER_FUN is in the callee's code (function decl),
                // so use self.current_loc which still points to the caller's last LOC
                let call_site = if is_inlined {
                    call_site_range
                } else {
                    self.current_loc.clone()
                };
                self.call_stack.push(CallFrame {
                    f_idx,
                    f_name: self.function_name_by_idx(f_idx),
                    is_inlined,
                    is_builtin,
                    call_site_loc: Some(call_site),
                    variables: Vec::new(),
                    scope_stack: Vec::new(),
                    pending_ir_return: None,
                });
            }
            Tick::PopFrame { .. } => {
                let popped = self.call_stack.pop();
                if let Some(frame) = popped
                    && !frame.is_inlined
                    && self.exec_stack.len() > 1
                {
                    self.exec_stack.pop();
                }
                // after leaving a function, we'll stop on the next expression inside caller;
                // discussable: shall we land exactly at expression where we called it?
                // if yes, "pending_ticks.push_front(Tick::Loc { range: call_site_loc })"
                // (but there will be a problem with mutate functions, we'll still see old values)
            }
            Tick::StackLayout { ir_stack: stack } => {
                let exec = self.exec_stack.last_mut().expect(
                    "replayer invariant: exec_stack must contain the active execution state",
                );
                if self.tvm_stack_values.len() >= stack.len() {
                    exec.system_stack_depth = self.tvm_stack_values.len() - stack.len();
                }
                if exec.accumulated_needs_reset {
                    exec.accumulated_ir_live = stack.iter().copied().collect();
                    exec.accumulated_needs_reset = false;
                } else {
                    exec.accumulated_ir_live.extend(stack.iter().copied());
                }
                exec.ir_stack = stack;
                self.update_last_seen();
            }
            Tick::TvmStackValues { values } => {
                self.clear_caught_exception();
                self.tvm_stack_values = values;
                let exec = self.exec_stack.last_mut().expect(
                    "replayer invariant: exec_stack must contain the active execution state",
                );
                exec.accumulated_needs_reset = true;
            }
            Tick::TvmBeforeExecute => {
                // stop before execution, not after (see below)
                if step_mode == StepMode::EachAsmInstruction {
                    return true;
                }
            }
            Tick::TvmAfterExecute { instr_name } => {
                // note: right after `EXECUTE INSTR`, tvm_stack_values and ir_stack are outdated
                // until fetched from vmlog (Tick::TvmStackValues) and marks (Tick::StackLayout);
                // (that's why in step_mode EachAsmInstruction we stop before execution, showing actual stack)
                self.prev_was_pushcont = instr_name.starts_with("PUSHCONT");
                if instr_name == "IFRET" || instr_name == "IFNOTRET" || instr_name == "RETALT" {
                    self.after_exception_ifret = true;
                }
            }
            Tick::TvmImplicitJmpRef => {}
            Tick::TvmException { errno } => {
                // "handling exception code N" from VM log; we don't yet what will happen next:
                // - if a TvmExceptionHandler tick follows — uncaught (VM terminates);
                // - if normal execution continues (Loc/TvmStackValues) — caught by try/catch.
                self.after_exception_ifret = true;
                if self.exception_break_mode != ExceptionBreakMode::Never {
                    self.last_exception = Some(ExceptionInfo {
                        errno,
                        is_uncaught: false,
                    });
                }
                return self.exception_break_mode == ExceptionBreakMode::All;
            }
            Tick::TvmExceptionHandler { .. } => {
                // "default exception handler, terminating vm with exit code N" from VM log.
                // This always follows TvmException when the exception is NOT caught by try/catch.
                if let Some(ref mut exc) = self.last_exception {
                    // last_exception may be None if mode=All (we already stopped, and step() cleared it)
                    exc.is_uncaught = true;
                }
                return self.exception_break_mode == ExceptionBreakMode::Uncaught;
            }
            Tick::LocalVar {
                var_name,
                ty_idx,
                ir_slots,
                is_lazy,
                ..
            } => {
                let frame = self.call_stack.last_mut().expect("no last frame");
                let new_var = LocalVarInScope {
                    name: var_name.clone(),
                    ty_idx,
                    ir_slots: ir_slots.clone(),
                    is_lazy,
                };
                let vars = frame.current_vars_mut();
                if let Some(existing) = vars.iter_mut().find(|v| v.name == var_name) {
                    existing.ir_slots = ir_slots;
                } else {
                    vars.push(new_var);
                }
            }
            Tick::SmartCast {
                var_name,
                ty_idx,
                ir_slots,
            } => {
                let frame = self.call_stack.last_mut().expect("no last frame");
                let found = frame
                    .scope_stack
                    .iter_mut()
                    .rev()
                    .flat_map(|s| s.variables.iter_mut())
                    .chain(frame.variables.iter_mut())
                    .find(|v| v.name == var_name);
                if let Some(existing) = found {
                    existing.ty_idx = ty_idx;
                    existing.ir_slots = ir_slots;
                    // if a variable's type is narrowed, a compiler will also report de-cast later
                }
            }
            Tick::SetGlob {
                glob_name,
                ty_idx,
                ir_slots,
            } => {
                let exec = self.exec_stack.last().expect(
                    "replayer invariant: exec_stack must contain the active execution state",
                );
                let captured: Vec<VmStackValue> = ir_slots
                    .iter()
                    .map(|&ir| {
                        exec.last_seen_values
                            .get(&ir)
                            .cloned()
                            .unwrap_or(VmStackValue::Unknown)
                    })
                    .collect();
                self.global_var_values.insert(glob_name, (ty_idx, captured));
            }
            Tick::ScopeStart { range } => {
                let frame = self.call_stack.last_mut().expect("no last frame");
                frame.scope_stack.push(LexicalScope {
                    range,
                    variables: Vec::new(),
                });
            }
            Tick::ScopeEnd => {
                let frame = self.call_stack.last_mut().expect("no last frame");
                frame.scope_stack.pop();
            }
        }
        false
    }

    /// Format leave-function return using the function's return type for rendering.
    pub fn format_leave_return(&self, f_idx: usize, ir_return: &[usize]) -> RenderedValue {
        let exec = self
            .exec_stack
            .last()
            .expect("replayer invariant: exec_stack must contain the active execution state");
        let values: Vec<SlotValue> = ir_return
            .iter()
            .map(|&ir_idx| {
                exec.last_seen_values
                    .get(&ir_idx)
                    .map(SlotValue::Live)
                    .unwrap_or(SlotValue::OptimizedOut)
            })
            .collect();

        let return_ty = self
            .source_map
            .get_function_by_idx(f_idx)
            .and_then(|f| self.source_map.resolve_ty(f.return_ty_idx));

        match return_ty {
            Some(ty) => debug_print_from_stack(&self.source_map, &values, ty),
            None => RenderedValue::Leaf("return type not found".to_string()),
        }
    }

    /// Snapshot current ir_stack→TVM value mappings so that variables whose
    /// slots disappear from stack can still be shown as "last seen".
    fn update_last_seen(&mut self) {
        let exec = self
            .exec_stack
            .last_mut()
            .expect("replayer invariant: exec_stack must contain the active execution state");
        let skip = exec.system_stack_depth.min(self.tvm_stack_values.len());
        for (i, &ir_idx) in exec.ir_stack.iter().enumerate() {
            if let Some(val) = self.tvm_stack_values.get(skip + i) {
                exec.last_seen_values.insert(ir_idx, val.clone());
            }
        }
    }

    /// Build the "locals" section for a specific call frame.
    fn format_locals_of(
        &self,
        frame: &CallFrame,
        last_seen: &HashMap<usize, VmStackValue>,
        ir_live: &HashSet<usize>,
    ) -> Vec<LocalVarRendered> {
        let mut result: Vec<LocalVarRendered> = Vec::new();

        for var in frame.all_visible_vars() {
            let slot_values: Vec<SlotValue> = var
                .ir_slots
                .iter()
                .map(|&ir| {
                    if let Some(val) = last_seen.get(&ir) {
                        if ir_live.contains(&ir) {
                            SlotValue::Live(val)
                        } else {
                            SlotValue::LastSeen(val)
                        }
                    } else {
                        SlotValue::OptimizedOut
                    }
                })
                .collect();

            let debug_val = if var.is_lazy {
                match self.source_map.resolve_ty(var.ty_idx) {
                    Some(ty) => debug_format_lazy(
                        &self.source_map,
                        &slot_values,
                        &var.ir_slots,
                        ty,
                        last_seen,
                    ),
                    None => RenderedValue::Leaf("var.ty_idx not found".to_string()),
                }
            } else {
                match self.source_map.resolve_ty(var.ty_idx) {
                    Some(ty) => debug_print_from_stack(&self.source_map, &slot_values, ty),
                    None => RenderedValue::Leaf("var.ty_idx not found".to_string()),
                }
            };
            result.push(LocalVarRendered {
                var_name: var.name.clone(),
                value: debug_val,
            });
        }

        for (name, (ty_idx, values)) in &self.global_var_values {
            let slot_values: Vec<SlotValue> = values.iter().map(SlotValue::Live).collect();
            let debug_val = match self.source_map.resolve_ty(*ty_idx) {
                Some(ty) => debug_print_from_stack(&self.source_map, &slot_values, ty),
                None => RenderedValue::Leaf("var.ty_idx not found".to_string()),
            };
            result.push(LocalVarRendered {
                var_name: format!("global {name}"),
                value: debug_val,
            });
        }

        if let Some(ir_return) = &frame.pending_ir_return {
            let return_val = self.format_leave_return(frame.f_idx, ir_return);
            result.push(LocalVarRendered {
                var_name: "(return value)".to_string(),
                value: return_val,
            });
        }

        result
    }

    /// If there is a pending exception that wasn't followed by TvmExceptionHandler,
    /// the exception was caught (try/catch). Clear the pending state.
    fn clear_caught_exception(&mut self) {
        self.last_exception = None;
    }

    /// TVM sometimes jumps to a location outside the current function
    /// without passing through MARK_LEAVE_FUN: exceptions caught by TRY,
    /// IFRET/IFNOTRET (Fift optimizes IFJMP:<{empty}> → IFRET), etc.
    fn unwind_after_exception_ifret(&mut self, range: &SrcRange) {
        self.after_exception_ifret = false;

        let loc_line = range.start_line();

        if let Some(top) = self.call_stack.last()
            && self.is_loc_within_function(top.f_idx, loc_line)
        {
            return;
        }

        while self.call_stack.len() > 1 {
            let top = self
                .call_stack
                .last()
                .expect("replayer invariant: call_stack should be non-empty while unwinding");
            if self.is_loc_within_function(top.f_idx, loc_line) {
                break;
            }
            let popped = self
                .call_stack
                .pop()
                .expect("replayer invariant: a frame should be present to unwind");
            if !popped.is_inlined && self.exec_stack.len() > 1 {
                let popped_exec = self.exec_stack.pop().expect(
                    "replayer invariant: a noinline exec state should exist when unwinding",
                );
                if let Some(parent_exec) = self.exec_stack.last_mut() {
                    parent_exec.ir_stack = popped_exec.ir_stack;
                    parent_exec.system_stack_depth = popped_exec.system_stack_depth;
                    parent_exec
                        .last_seen_values
                        .extend(popped_exec.last_seen_values);
                }
            }
        }

        if let Some(frame) = self.call_stack.last_mut() {
            while let Some(scope) = frame.scope_stack.last() {
                if loc_line >= scope.range.start_line() && loc_line <= scope.range.end_line() {
                    break;
                }
                frame.scope_stack.pop();
            }
        }
    }

    /// Whether the debugger should stop at the current location.
    /// `at_fun_return` is true when we are at MARK_LEAVE_FUN (Tick::AtFunReturn).
    fn should_stop(&self, step_mode: StepMode, at_fun_return: bool) -> bool {
        let new_line = self.current_loc.start_line();
        let file_id = self.current_loc.file_id();

        // we always stop at any breakpoint (but prevent multiple hits of the same breakpoint)
        let at_breakpoint = self.breakpoints.contains(&(file_id, new_line));
        if at_breakpoint && new_line != self.breakpoint_skip_line {
            return true;
        }

        // "step into" also stops at "return" statement, to inspect "(return value)" in locals
        if at_fun_return && step_mode == StepMode::StepInto {
            return true;
        }

        // analyze by depth
        let depth = self.call_stack.len();
        match step_mode {
            StepMode::StepOver => new_line != self.last_stop_line && depth <= self.last_stop_depth,
            StepMode::StepInto => new_line != self.last_stop_line || depth > self.last_stop_depth,
            StepMode::StepOut => depth < self.last_stop_depth,
            _ => false,
        }
    }

    /// Check if a source line falls within the declared range of a function.
    fn is_loc_within_function(&self, f_idx: usize, line: usize) -> bool {
        if let Some(func) = self.source_map.get_function_by_idx(f_idx) {
            return line >= func.ident_loc.start_line() && line <= func.end_loc.end_line();
        }
        true
    }

    /// Short display name (just the filename component).
    pub fn file_display_name(&self, file_id: usize) -> &str {
        self.source_map.resolve_file_name(file_id)
    }

    /// Resolve a path (from DAP: file:///... or /abs/path) to file_id.
    pub fn file_id_by_path(&self, path: &str) -> Option<usize> {
        self.source_map.path_to_file_id(path)
    }

    pub fn type_name(&self, ty_idx: usize) -> Option<String> {
        self.source_map.resolve_ty(ty_idx).map(|ty| ty.to_string())
    }
}
