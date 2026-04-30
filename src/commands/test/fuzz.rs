use super::{TestDescriptor, TestResult, TestRunner, evaluate_test_case};
use crate::commands::test::reporting::{FuzzCaseContext, FuzzExecutionContext};
use crate::context::{AssertFailure, FailAssertFailure, to_cell};
use acton_config::test::TestConfig;
use num_bigint::{BigInt, Sign};
use rand::rngs::StdRng;
use rand::{Rng, RngCore, SeedableRng};
use std::sync::Arc;
use tolk_compiler::SourceMap;
use tolk_compiler::abi::{ABIFunctionParameter, ContractABI, Ty};
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::cell::{Cell, HashBytes};
use tycho_types::models::{Base64StdAddrFlags, DisplayBase64StdAddr, StdAddr};

const DEFAULT_FUZZ_RUNS: usize = 256;
const DEFAULT_FUZZ_REJECT_BUDGET_MULTIPLIER: usize = 256;

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FuzzConfig {
    pub runs: Option<usize>,
    pub max_test_rejects: Option<usize>,
    pub seed: Option<u64>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct ResolvedFuzzConfig {
    runs: usize,
    max_test_rejects: usize,
    seed: Option<u64>,
}

#[derive(Debug, Clone)]
pub(super) struct FuzzParameter {
    name: String,
    type_name: String,
    kind: FuzzParameterKind,
}

#[derive(Debug, Clone)]
enum FuzzParameterKind {
    Int { signed: bool, bits: Option<usize> },
    Bool,
    String,
    Address,
    AnyAddress,
    Nullable(Box<FuzzParameterKind>),
    Unsupported,
}

#[derive(Debug)]
struct GeneratedFuzzInput {
    stack: Tuple,
    inputs: Vec<(String, String)>,
}

impl TestRunner<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn execute_fuzz_test(
        &mut self,
        test: &TestDescriptor,
        code_cell: &Cell,
        dest_address: &str,
        abi: Option<Arc<ContractABI>>,
        source_map: Arc<SourceMap>,
        fuzz: FuzzConfig,
    ) -> anyhow::Result<TestResult> {
        let fuzz = resolve_fuzz_config(fuzz, &self.config);
        let seed = fuzz.seed.unwrap_or(self.fuzz_seed);
        let mut executed_get_methods = Vec::new();
        let mut last_result: Option<TestResult> = None;
        let mut last_rejected_result: Option<TestResult> = None;
        let mut accepted_runs = 0usize;
        let mut rejected_runs = 0usize;
        let mut attempt_idx = 0usize;

        while accepted_runs < fuzz.runs {
            if rejected_runs >= fuzz.max_test_rejects {
                let Some(mut result) = last_rejected_result.take() else {
                    anyhow::bail!(
                        "Fuzz test '{}' exhausted assume(...) budget without executing any runs",
                        test.name
                    );
                };

                let location = match result.assert_failure.as_ref() {
                    Some(AssertFailure::Assume(failure)) => failure.location.clone(),
                    _ => None,
                };
                result.assert_failure = Some(AssertFailure::Assume(FailAssertFailure {
                    message: Some(format!(
                        "assume(...) rejected {rejected_runs} fuzz inputs before reaching {} successful runs (completed {accepted_runs})",
                        fuzz.runs
                    )),
                    location,
                }));
                result.executed_get_methods = executed_get_methods;
                result.fuzz = Some(FuzzExecutionContext {
                    total_runs: fuzz.runs,
                    seed,
                    failed_case: None,
                });
                return Ok(result);
            }

            let generated = generate_fuzz_input(test, attempt_idx, seed)?;
            attempt_idx += 1;
            let mut result = self.execute_test_case(
                test,
                code_cell,
                dest_address,
                abi.clone(),
                source_map.clone(),
                &generated.stack,
            )?;
            if self.config.coverage {
                executed_get_methods.append(&mut result.executed_get_methods);
            }

            if matches!(
                result.assert_failure.as_ref(),
                Some(AssertFailure::Assume(_))
            ) {
                rejected_runs += 1;
                last_rejected_result = Some(result);
                continue;
            }

            accepted_runs += 1;
            let outcome = evaluate_test_case(
                test,
                &result.get_result,
                result.assert_failure.as_ref(),
                result.expected_exit_code,
            );

            if !outcome.passed {
                result.executed_get_methods = executed_get_methods;
                result.fuzz = Some(FuzzExecutionContext {
                    total_runs: fuzz.runs,
                    seed,
                    failed_case: Some(FuzzCaseContext {
                        run: accepted_runs,
                        inputs: generated.inputs,
                    }),
                });
                return Ok(result);
            }

            last_result = Some(result);
        }

        let Some(mut result) = last_result else {
            anyhow::bail!("Fuzz test '{}' did not execute any runs", test.name);
        };
        result.executed_get_methods = executed_get_methods;
        result.fuzz = Some(FuzzExecutionContext {
            total_runs: fuzz.runs,
            seed,
            failed_case: None,
        });
        Ok(result)
    }
}

fn resolve_fuzz_config(fuzz: FuzzConfig, config: &TestConfig) -> ResolvedFuzzConfig {
    let runs = fuzz.runs.or(config.fuzz_runs).unwrap_or(DEFAULT_FUZZ_RUNS);
    let max_test_rejects = fuzz
        .max_test_rejects
        .or(config.fuzz_max_test_rejects)
        .unwrap_or_else(|| runs.saturating_mul(DEFAULT_FUZZ_REJECT_BUDGET_MULTIPLIER));

    ResolvedFuzzConfig {
        runs,
        max_test_rejects,
        seed: fuzz.seed,
    }
}

fn generate_fuzz_input(
    test: &TestDescriptor,
    run_idx: usize,
    seed: u64,
) -> anyhow::Result<GeneratedFuzzInput> {
    let run_salt = (run_idx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut rng = StdRng::seed_from_u64(seed ^ run_salt);
    let mut items = Vec::with_capacity(test.parameters.len());
    let mut inputs = Vec::with_capacity(test.parameters.len());

    for (param_index, parameter) in test.parameters.iter().enumerate() {
        let (item, display) =
            generate_fuzz_value(&parameter.kind, run_idx + param_index, &mut rng)?;
        items.push(item);
        inputs.push((parameter.name.clone(), display));
    }

    Ok(GeneratedFuzzInput {
        stack: Tuple(items),
        inputs,
    })
}

fn generate_fuzz_value(
    kind: &FuzzParameterKind,
    run_idx: usize,
    rng: &mut StdRng,
) -> anyhow::Result<(TupleItem, String)> {
    match kind {
        FuzzParameterKind::Int { signed, bits } => {
            let value = if *signed {
                generate_signed_integer(run_idx, *bits, rng)
            } else {
                generate_unsigned_integer(run_idx, *bits, rng)
            };
            Ok((TupleItem::Int(value.clone()), value.to_string()))
        }
        FuzzParameterKind::Bool => {
            let value = if run_idx == 0 {
                false
            } else if run_idx == 1 {
                true
            } else {
                rng.gen_bool(0.5)
            };
            let item = if value {
                TupleItem::Int(BigInt::from(-1))
            } else {
                TupleItem::Int(BigInt::ZERO)
            };
            Ok((item, value.to_string()))
        }
        FuzzParameterKind::String => {
            const CHARSET: &[u8] =
                b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";

            let len = match run_idx {
                0 => 0,
                1 => 1,
                2 => 5,
                _ => rng.gen_range(0..=24),
            };
            let value = (0..len)
                .map(|_| {
                    let idx = rng.gen_range(0..CHARSET.len());
                    CHARSET[idx] as char
                })
                .collect::<String>();
            let mut tuple = Tuple::empty();
            tuple.push_string_slice(&value);
            let item = tuple.pop().unwrap_or(TupleItem::Null);
            Ok((item, format!("{value:?}")))
        }
        FuzzParameterKind::Address | FuzzParameterKind::AnyAddress => {
            let address = generate_std_address(run_idx, rng);
            Ok((
                TupleItem::Cell(to_cell(&address)),
                format_std_address(&address),
            ))
        }
        FuzzParameterKind::Nullable(inner) => {
            if run_idx.is_multiple_of(4) {
                return Ok((TupleItem::Null, "null".to_owned()));
            }
            generate_fuzz_value(inner, run_idx - 1, rng)
        }
        FuzzParameterKind::Unsupported => anyhow::bail!("Unsupported fuzz parameter type"),
    }
}

fn generate_unsigned_integer(run_idx: usize, bits: Option<usize>, rng: &mut StdRng) -> BigInt {
    let bits = bits.unwrap_or(128);
    if bits == 0 {
        return BigInt::ZERO;
    }

    let max = (BigInt::from(1u8) << bits) - 1u8;
    match run_idx {
        0 => BigInt::ZERO,
        1 => BigInt::from(1u8),
        2 => max,
        3 if bits > 1 => BigInt::from(1u8) << (bits - 1),
        _ => random_positive_bigint(rng, bits),
    }
}

fn generate_signed_integer(run_idx: usize, bits: Option<usize>, rng: &mut StdRng) -> BigInt {
    let magnitude_bits = bits.map_or(127, |value| value.saturating_sub(1));
    let max = if magnitude_bits == 0 {
        BigInt::ZERO
    } else {
        (BigInt::from(1u8) << magnitude_bits) - 1u8
    };
    let min = if let Some(bits) = bits {
        if bits == 0 {
            BigInt::ZERO
        } else {
            -(BigInt::from(1u8) << (bits - 1))
        }
    } else {
        -(BigInt::from(1u8) << 127usize)
    };

    match run_idx {
        0 => BigInt::ZERO,
        1 => {
            if magnitude_bits == 0 {
                BigInt::ZERO
            } else {
                BigInt::from(1u8)
            }
        }
        2 => BigInt::from(-1),
        3 => max,
        4 => min,
        _ => {
            let value = random_positive_bigint(rng, magnitude_bits);
            if rng.gen_bool(0.5) { -value } else { value }
        }
    }
}

fn random_positive_bigint(rng: &mut StdRng, bits: usize) -> BigInt {
    if bits == 0 {
        return BigInt::ZERO;
    }

    let mut raw = vec![0u8; bits.div_ceil(8)];
    rng.fill_bytes(&mut raw);
    mask_high_bits(&mut raw, bits);
    BigInt::from_bytes_be(Sign::Plus, &raw)
}

fn mask_high_bits(raw: &mut [u8], bits: usize) {
    if raw.is_empty() || bits == 0 {
        return;
    }

    let extra_bits = raw.len() * 8 - bits;
    if extra_bits > 0 {
        raw[0] &= 0xFF_u8 >> extra_bits;
    }
}

fn generate_std_address(run_idx: usize, rng: &mut StdRng) -> StdAddr {
    let mut hash = [0u8; 32];
    match run_idx {
        0 => {}
        1 => hash.fill(0xFF),
        2 => {
            for (index, byte) in hash.iter_mut().enumerate() {
                *byte = index as u8;
            }
        }
        _ => rng.fill_bytes(&mut hash),
    }

    let workchain = if run_idx.is_multiple_of(2) { 0i8 } else { -1i8 };
    StdAddr::new(workchain, HashBytes(hash))
}

fn format_std_address(address: &StdAddr) -> String {
    DisplayBase64StdAddr {
        addr: address,
        flags: Base64StdAddrFlags {
            testnet: false,
            base64_url: true,
            bounceable: true,
        },
    }
    .to_string()
}

pub(super) fn attach_test_parameter_metadata(
    mut tests: Vec<TestDescriptor>,
    abi: Option<&ContractABI>,
) -> Vec<TestDescriptor> {
    for test in &mut tests {
        if let Some(method) = abi.and_then(|abi| {
            abi.get_methods
                .iter()
                .find(|method| method.tvm_method_id == test.id || method.name == test.name.as_ref())
        }) {
            test.parameters = method
                .parameters
                .iter()
                .map(map_compiler_parameter)
                .collect();
        }
    }

    tests
}

fn map_compiler_parameter(parameter: &ABIFunctionParameter) -> FuzzParameter {
    FuzzParameter {
        name: parameter.name.clone(),
        type_name: parameter.ty.render_type(),
        kind: map_compiler_type(&parameter.ty),
    }
}

fn map_compiler_type(ty: &Ty) -> FuzzParameterKind {
    match ty {
        Ty::Int => FuzzParameterKind::Int {
            signed: true,
            bits: None,
        },
        Ty::Coins => FuzzParameterKind::Int {
            signed: false,
            bits: Some(120),
        },
        Ty::UintN { n } => FuzzParameterKind::Int {
            signed: false,
            bits: Some(*n as usize),
        },
        Ty::VaruintN { n } => match variadic_integer_payload_bits(*n as usize) {
            Some(bits) => FuzzParameterKind::Int {
                signed: false,
                bits: Some(bits),
            },
            None => FuzzParameterKind::Unsupported,
        },
        Ty::IntN { n } => FuzzParameterKind::Int {
            signed: true,
            bits: Some(*n as usize),
        },
        Ty::VarintN { n } => match variadic_integer_payload_bits(*n as usize) {
            Some(bits) => FuzzParameterKind::Int {
                signed: true,
                bits: Some(bits),
            },
            None => FuzzParameterKind::Unsupported,
        },
        Ty::Bool => FuzzParameterKind::Bool,
        Ty::String => FuzzParameterKind::String,
        Ty::Address => FuzzParameterKind::Address,
        Ty::AddressAny => FuzzParameterKind::AnyAddress,
        Ty::AddressOpt => FuzzParameterKind::Nullable(Box::new(FuzzParameterKind::Address)),
        Ty::Nullable { inner, .. } => match map_compiler_type(inner) {
            FuzzParameterKind::Unsupported => FuzzParameterKind::Unsupported,
            inner => FuzzParameterKind::Nullable(Box::new(inner)),
        },
        _ => FuzzParameterKind::Unsupported,
    }
}

fn variadic_integer_payload_bits(size: usize) -> Option<usize> {
    if size == 0 || !size.is_power_of_two() {
        return None;
    }

    size.checked_sub(1)?.checked_mul(8)
}

pub(super) fn validate_test_configuration(
    test: &TestDescriptor,
    config: &TestConfig,
) -> anyhow::Result<()> {
    if test.fuzz.is_some() && test.declared_parameter_count == 0 {
        anyhow::bail!(
            "Test '{}' uses @test.fuzz(...) but has no parameters",
            test.name
        );
    }

    if test.declared_parameter_count > 0 && test.fuzz.is_none() {
        anyhow::bail!(
            "Parameterized test '{}' requires @test.fuzz, @test.fuzz(<runs>), or @test.fuzz({{ ... }})",
            test.name
        );
    }

    let Some(fuzz) = test.fuzz else {
        return Ok(());
    };

    let fuzz = resolve_fuzz_config(fuzz, config);

    if fuzz.runs == 0 {
        anyhow::bail!("Fuzz runs must be greater than 0 for test '{}'", test.name);
    }

    if fuzz.max_test_rejects == 0 {
        anyhow::bail!(
            "Fuzz max-test-rejects must be greater than 0 for test '{}'",
            test.name
        );
    }

    if test.parameters.len() != test.declared_parameter_count {
        anyhow::bail!(
            "Cannot derive parameter metadata for fuzz test '{}'",
            test.name
        );
    }

    if let Some(parameter) = test
        .parameters
        .iter()
        .find(|parameter| matches!(parameter.kind, FuzzParameterKind::Unsupported))
    {
        anyhow::bail!(
            "Fuzzing parameter '{}' of type '{}' is not supported yet in test '{}'",
            parameter.name,
            parameter.type_name,
            test.name
        );
    }

    Ok(())
}
