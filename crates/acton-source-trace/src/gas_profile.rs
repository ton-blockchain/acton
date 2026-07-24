use crate::CompiledTolkSourceTrace;
use acton_debug::replayer::{CallFrameInfo, StepMode, Tick, TolkReplayer};
use serde::{Deserialize, Serialize};
use tolk_source_map::SourceMap;
use tolk_source_map::source_map::SrcRange;
use tvm_logs::gas::{DEFAULT_INITIAL_GAS, GasTracker};
use tvm_logs::parser::{VmLine, parse_lines};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildCompiledGasProfileRequest {
    pub code_hash: String,
    pub compiled: CompiledTolkSourceTrace,
    pub executions: Vec<GasProfileExecutionRequest>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GasProfileExecutionRequest {
    pub id: String,
    pub vm_logs: String,
    pub initial_gas: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_name: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GasProfileResponse {
    pub code_hash: String,
    pub executions: Vec<GasProfileExecutionResponse>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GasProfileExecutionResponse {
    pub id: String,
    pub total_gas: u64,
    pub sample_count: usize,
    pub samples: Vec<GasProfileSample>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GasProfileSample {
    pub instruction_name: String,
    pub frames: Vec<GasProfileFrame>,
    pub weight: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GasProfileFrame {
    pub function_name: String,
    pub url: String,
    pub line_number: i64,
    pub column_number: i64,
}

#[derive(Debug)]
struct InstructionGasStep {
    instruction_name: String,
    gas: u64,
}

pub fn build_gas_profile_response(
    code_hash: String,
    source_map: &SourceMap,
    executions: Vec<GasProfileExecutionRequest>,
) -> GasProfileResponse {
    let executions = executions
        .into_iter()
        .map(|execution| {
            let samples = collect_gas_profile_samples(
                &execution.vm_logs,
                Some(execution.initial_gas),
                source_map,
                execution.contract_name.as_deref(),
            );
            GasProfileExecutionResponse {
                id: execution.id,
                total_gas: samples.iter().map(|sample| sample.weight).sum(),
                sample_count: samples.len(),
                samples,
            }
        })
        .collect();

    GasProfileResponse {
        code_hash,
        executions,
    }
}

#[must_use]
pub fn collect_gas_profile_samples(
    vm_logs: &str,
    initial_gas: Option<u64>,
    source_map: &SourceMap,
    contract_name: Option<&str>,
) -> Vec<GasProfileSample> {
    let execute_steps = instruction_gas_steps(vm_logs, initial_gas);
    if execute_steps.is_empty() {
        return Vec::new();
    }

    let Ok(mut replayer) = TolkReplayer::new(source_map, vm_logs) else {
        return Vec::new();
    };
    let mut samples = Vec::new();
    let mut execute_idx = 0usize;

    replayer.step_with_callback(StepMode::RunUntilBreakpoint, |tick, state| match tick {
        Tick::TvmImplicitJmpRef => {
            if let Some(sample) = record_execution_sample(
                contract_name,
                &execute_steps,
                &mut execute_idx,
                state,
                Some("implicit JMPREF"),
            ) {
                samples.push(sample);
            }
        }
        Tick::TvmBeforeExecute => {
            while execute_steps
                .get(execute_idx)
                .is_some_and(|step| step.instruction_name == "implicit JMPREF")
            {
                if let Some(sample) = record_execution_sample(
                    contract_name,
                    &execute_steps,
                    &mut execute_idx,
                    state,
                    Some("implicit JMPREF"),
                ) {
                    samples.push(sample);
                }
            }

            if let Some(sample) = record_execution_sample(
                contract_name,
                &execute_steps,
                &mut execute_idx,
                state,
                None,
            ) {
                samples.push(sample);
            }
        }
        _ => {}
    });

    samples
}

fn instruction_gas_steps(vm_logs: &str, initial_gas: Option<u64>) -> Vec<InstructionGasStep> {
    let initial_gas = initial_gas
        .and_then(|gas| usize::try_from(gas).ok())
        .unwrap_or(DEFAULT_INITIAL_GAS);
    let mut gas_tracker = GasTracker::new(initial_gas);
    let mut current_instruction = None;
    let mut steps = Vec::new();

    for line in parse_lines(vm_logs).filter_map(Result::ok) {
        match &line {
            VmLine::VmExecute { instr } => current_instruction = Some((*instr).to_owned()),
            VmLine::VmGasRemaining { .. } => {
                let gas = gas_tracker.update(&line).unwrap_or_default() as u64;
                steps.push(InstructionGasStep {
                    instruction_name: current_instruction.take().unwrap_or_default(),
                    gas,
                });
            }
            VmLine::VmLimitChanged { .. } => {
                let _ = gas_tracker.update(&line);
            }
            _ => {}
        }
    }

    steps
}

fn record_execution_sample(
    contract_name: Option<&str>,
    execute_steps: &[InstructionGasStep],
    execute_idx: &mut usize,
    replayer: &TolkReplayer,
    expected_instruction: Option<&str>,
) -> Option<GasProfileSample> {
    let step = execute_steps.get(*execute_idx)?;

    if let Some(expected_instruction) = expected_instruction
        && step.instruction_name != expected_instruction
    {
        return None;
    }

    *execute_idx += 1;
    if step.gas == 0 {
        return None;
    }

    let frames = replayer
        .call_stack()
        .iter()
        .map(|frame| GasProfileFrame::from_call_frame(frame, contract_name, replayer))
        .collect::<Vec<_>>();
    if frames.is_empty() {
        return None;
    }

    Some(GasProfileSample {
        instruction_name: profile_instruction_name(&step.instruction_name),
        frames,
        weight: step.gas,
    })
}

fn profile_instruction_name(instruction_name: &str) -> String {
    if instruction_name.starts_with("implicit ") {
        return instruction_name.to_owned();
    }

    instruction_name
        .split_whitespace()
        .next()
        .unwrap_or(instruction_name)
        .to_owned()
}

impl GasProfileFrame {
    fn from_call_frame(
        frame: &CallFrameInfo,
        contract_name: Option<&str>,
        replayer: &TolkReplayer,
    ) -> Self {
        let location = frame
            .definition_loc
            .as_ref()
            .or(frame.call_site_loc.as_ref());
        let (url, line_number, column_number) = location.map_or_else(
            || (String::new(), -1, -1),
            |location| frame_location(replayer, location),
        );

        Self {
            function_name: format_profile_function_name(frame.f_name.as_str(), contract_name),
            url,
            line_number,
            column_number,
        }
    }
}

fn format_profile_function_name(function_name: &str, contract_name: Option<&str>) -> String {
    if matches!(
        function_name,
        "onInternalMessage" | "onExternalMessage" | "onBouncedMessage" | "onRunTickTock"
    ) && let Some(contract_name) = contract_name
    {
        return format!("{contract_name}:{function_name}");
    }

    function_name.to_owned()
}

fn frame_location(replayer: &TolkReplayer, range: &SrcRange) -> (String, i64, i64) {
    let url = replayer.file_full_path(range.file_id()).unwrap_or_default();

    (
        url.to_string(),
        zero_based_position(range.start_line()),
        zero_based_position(range.start_col()),
    )
}

fn zero_based_position(position: usize) -> i64 {
    position
        .checked_sub(1)
        .map_or(-1, |position| position as i64)
}
