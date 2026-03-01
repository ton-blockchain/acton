#![allow(clippy::redundant_closure_call)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::match_single_binding)]
#![allow(clippy::clone_on_copy)]

use std::convert::Infallible;

pub const TVM_SPEC_JSON: &str = include_str!("../spec/tvm-specification.json");

pub fn load_tvm_specification() -> serde_json::Result<Specification> {
    serde_json::from_str(TVM_SPEC_JSON)
}

pub mod error {
    pub struct ConversionError(std::borrow::Cow<'static, str>);
    impl std::error::Error for ConversionError {}
    impl std::fmt::Display for ConversionError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
            std::fmt::Display::fmt(&self.0, f)
        }
    }
    impl std::fmt::Debug for ConversionError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
            std::fmt::Debug::fmt(&self.0, f)
        }
    }
    impl From<&'static str> for ConversionError {
        fn from(value: &'static str) -> Self {
            Self(value.into())
        }
    }
    impl From<String> for ConversionError {
        fn from(value: String) -> Self {
            Self(value.into())
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(tag = "$")]
pub enum Arg {
    #[serde(rename = "uint")]
    UintArg(UintArg),
    #[serde(rename = "int")]
    IntArg(IntArg),
    #[serde(rename = "delta")]
    DeltaArg(DeltaArg),
    #[serde(rename = "stack")]
    StackArg(StackArg),
    #[serde(rename = "control")]
    ControlArg(ControlArg),
    #[serde(rename = "plduzArg")]
    PlduzArg(PlduzArg),
    #[serde(rename = "tinyInt")]
    TinyIntArg(TinyIntArg),
    #[serde(rename = "largeInt")]
    LargeIntArg(LargeIntArg),
    #[serde(rename = "minusOne")]
    MinusOneArg(MinusOneArg),
    #[serde(rename = "s1")]
    S1Arg(S1Arg),
    #[serde(rename = "setcpArg")]
    SetcpArg(SetcpArg),
    #[serde(rename = "slice")]
    SliceArg(SliceArg),
    #[serde(rename = "codeSlice")]
    CodeSliceArg(CodeSliceArg),
    #[serde(rename = "refCodeSlice")]
    RefCodeSliceArg(RefCodeSliceArg),
    #[serde(rename = "inlineCodeSlice")]
    InlineCodeSliceArg(InlineCodeSliceArg),
    #[serde(rename = "dict")]
    InlineDictArg(InlineDictArg),
    #[serde(rename = "exoticCell")]
    ExoticCellArg(ExoticCellArg),
    #[serde(rename = "debugstr")]
    DebugstrArg(DebugstrArg),
}
impl From<&Self> for Arg {
    fn from(value: &Arg) -> Self {
        value.clone()
    }
}
impl From<UintArg> for Arg {
    fn from(value: UintArg) -> Self {
        Self::UintArg(value)
    }
}
impl From<IntArg> for Arg {
    fn from(value: IntArg) -> Self {
        Self::IntArg(value)
    }
}
impl From<DeltaArg> for Arg {
    fn from(value: DeltaArg) -> Self {
        Self::DeltaArg(value)
    }
}
impl From<StackArg> for Arg {
    fn from(value: StackArg) -> Self {
        Self::StackArg(value)
    }
}
impl From<ControlArg> for Arg {
    fn from(value: ControlArg) -> Self {
        Self::ControlArg(value)
    }
}
impl From<PlduzArg> for Arg {
    fn from(value: PlduzArg) -> Self {
        Self::PlduzArg(value)
    }
}
impl From<TinyIntArg> for Arg {
    fn from(value: TinyIntArg) -> Self {
        Self::TinyIntArg(value)
    }
}
impl From<LargeIntArg> for Arg {
    fn from(value: LargeIntArg) -> Self {
        Self::LargeIntArg(value)
    }
}
impl From<MinusOneArg> for Arg {
    fn from(value: MinusOneArg) -> Self {
        Self::MinusOneArg(value)
    }
}
impl From<S1Arg> for Arg {
    fn from(value: S1Arg) -> Self {
        Self::S1Arg(value)
    }
}
impl From<SetcpArg> for Arg {
    fn from(value: SetcpArg) -> Self {
        Self::SetcpArg(value)
    }
}
impl From<SliceArg> for Arg {
    fn from(value: SliceArg) -> Self {
        Self::SliceArg(value)
    }
}
impl From<CodeSliceArg> for Arg {
    fn from(value: CodeSliceArg) -> Self {
        Self::CodeSliceArg(value)
    }
}
impl From<RefCodeSliceArg> for Arg {
    fn from(value: RefCodeSliceArg) -> Self {
        Self::RefCodeSliceArg(value)
    }
}
impl From<InlineCodeSliceArg> for Arg {
    fn from(value: InlineCodeSliceArg) -> Self {
        Self::InlineCodeSliceArg(value)
    }
}
impl From<InlineDictArg> for Arg {
    fn from(value: InlineDictArg) -> Self {
        Self::InlineDictArg(value)
    }
}
impl From<ExoticCellArg> for Arg {
    fn from(value: ExoticCellArg) -> Self {
        Self::ExoticCellArg(value)
    }
}
impl From<DebugstrArg> for Arg {
    fn from(value: DebugstrArg) -> Self {
        Self::DebugstrArg(value)
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ArgRange {
    pub max: String,

    pub min: String,
}
impl From<&ArgRange> for ArgRange {
    fn from(value: &ArgRange) -> Self {
        value.clone()
    }
}
impl ArgRange {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct Args(pub Vec<Arg>);
impl std::ops::Deref for Args {
    type Target = Vec<Arg>;
    fn deref(&self) -> &Vec<Arg> {
        &self.0
    }
}
impl From<Args> for Vec<Arg> {
    fn from(value: Args) -> Self {
        value.0
    }
}
impl From<&Args> for Args {
    fn from(value: &Args) -> Self {
        value.clone()
    }
}
impl From<Vec<Arg>> for Args {
    fn from(value: Vec<Arg>) -> Self {
        Self(value)
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct ArmValue(pub i64);
impl std::ops::Deref for ArmValue {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl From<ArmValue> for i64 {
    fn from(value: ArmValue) -> Self {
        value.0
    }
}
impl From<&ArmValue> for ArmValue {
    fn from(value: &ArmValue) -> Self {
        value.clone()
    }
}
impl From<i64> for ArmValue {
    fn from(value: i64) -> Self {
        Self(value)
    }
}
impl std::str::FromStr for ArmValue {
    type Err = <i64 as std::str::FromStr>::Err;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.parse()?))
    }
}
impl TryFrom<&str> for ArmValue {
    type Error = <i64 as std::str::FromStr>::Err;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.parse()
    }
}
impl TryFrom<&String> for ArmValue {
    type Error = <i64 as std::str::FromStr>::Err;
    fn try_from(value: &String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
impl TryFrom<String> for ArmValue {
    type Error = <i64 as std::str::FromStr>::Err;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
impl std::fmt::Display for ArmValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct ArraySingleEntryDefinition(pub StackValues);
impl std::ops::Deref for ArraySingleEntryDefinition {
    type Target = StackValues;
    fn deref(&self) -> &StackValues {
        &self.0
    }
}
impl From<ArraySingleEntryDefinition> for StackValues {
    fn from(value: ArraySingleEntryDefinition) -> Self {
        value.0
    }
}
impl From<&ArraySingleEntryDefinition> for ArraySingleEntryDefinition {
    fn from(value: &ArraySingleEntryDefinition) -> Self {
        value.clone()
    }
}
impl From<StackValues> for ArraySingleEntryDefinition {
    fn from(value: StackValues) -> Self {
        Self(value)
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CodeSliceArg {
    pub bits: Box<Arg>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    pub refs: Box<Arg>,
}
impl From<&CodeSliceArg> for CodeSliceArg {
    fn from(value: &CodeSliceArg) -> Self {
        value.clone()
    }
}
impl CodeSliceArg {}

#[derive(
    serde::Deserialize, serde::Serialize, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
pub enum ConstantType {
    Int,
    Null,
}
impl From<&Self> for ConstantType {
    fn from(value: &ConstantType) -> Self {
        value.clone()
    }
}
impl std::fmt::Display for ConstantType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Int => f.write_str("Int"),
            Self::Null => f.write_str("Null"),
        }
    }
}
impl std::str::FromStr for ConstantType {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> Result<Self, error::ConversionError> {
        match value {
            "Int" => Ok(Self::Int),
            "Null" => Ok(Self::Null),
            _ => Err("invalid value".into()),
        }
    }
}
impl TryFrom<&str> for ConstantType {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl TryFrom<&String> for ConstantType {
    type Error = error::ConversionError;
    fn try_from(value: &String) -> Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl TryFrom<String> for ConstantType {
    type Error = error::ConversionError;
    fn try_from(value: String) -> Result<Self, error::ConversionError> {
        value.parse()
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum ConstantValue {
    Integer(i64),
    String(String),
    Null,
}
impl From<&Self> for ConstantValue {
    fn from(value: &ConstantValue) -> Self {
        value.clone()
    }
}
impl From<i64> for ConstantValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(untagged, deny_unknown_fields)]
pub enum Continuation {
    Variant0 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        save: Option<Savelist>,
        #[serde(rename = "type")]
        type_: ::serde_json::Value,
    },
    Variant1 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        save: Option<Savelist>,
        #[serde(rename = "type")]
        type_: ::serde_json::Value,
        var_name: VariableName,
    },
    Variant2 {
        index: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        save: Option<Savelist>,
        #[serde(rename = "type")]
        type_: ::serde_json::Value,
    },
    Variant3 {
        args: ContinuationVariant3Args,
        name: ::serde_json::Value,
        #[serde(rename = "type")]
        type_: ::serde_json::Value,
    },
    Variant4 {
        args: ContinuationVariant4Args,
        name: ::serde_json::Value,
        #[serde(rename = "type")]
        type_: ::serde_json::Value,
    },
    Variant5 {
        args: ContinuationVariant5Args,
        name: ::serde_json::Value,
        #[serde(rename = "type")]
        type_: ::serde_json::Value,
    },
    Variant6 {
        args: ContinuationVariant6Args,
        name: ::serde_json::Value,
        #[serde(rename = "type")]
        type_: ::serde_json::Value,
    },
    Variant7 {
        args: ContinuationVariant7Args,
        name: ::serde_json::Value,
        #[serde(rename = "type")]
        type_: ::serde_json::Value,
    },
}
impl From<&Self> for Continuation {
    fn from(value: &Continuation) -> Self {
        value.clone()
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ContinuationVariant3Args {
    pub after: Box<Continuation>,
    pub body: Box<Continuation>,
}
impl From<&ContinuationVariant3Args> for ContinuationVariant3Args {
    fn from(value: &ContinuationVariant3Args) -> Self {
        value.clone()
    }
}
impl ContinuationVariant3Args {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ContinuationVariant4Args {
    pub after: Box<Continuation>,
    pub body: Box<Continuation>,
    pub cond: Box<Continuation>,
}
impl From<&ContinuationVariant4Args> for ContinuationVariant4Args {
    fn from(value: &ContinuationVariant4Args) -> Self {
        value.clone()
    }
}
impl ContinuationVariant4Args {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ContinuationVariant5Args {
    pub body: Box<Continuation>,
}
impl From<&ContinuationVariant5Args> for ContinuationVariant5Args {
    fn from(value: &ContinuationVariant5Args) -> Self {
        value.clone()
    }
}
impl ContinuationVariant5Args {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ContinuationVariant6Args {
    pub after: Box<Continuation>,
    pub body: Box<Continuation>,
    pub count: VariableName,
}
impl From<&ContinuationVariant6Args> for ContinuationVariant6Args {
    fn from(value: &ContinuationVariant6Args) -> Self {
        value.clone()
    }
}
impl ContinuationVariant6Args {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ContinuationVariant7Args {
    pub next: Box<Continuation>,
    pub value: i64,
}
impl From<&ContinuationVariant7Args> for ContinuationVariant7Args {
    fn from(value: &ContinuationVariant7Args) -> Self {
        value.clone()
    }
}
impl ContinuationVariant7Args {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ControlArg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    pub range: ArgRange,
}
impl From<&ControlArg> for ControlArg {
    fn from(value: &ControlArg) -> Self {
        value.clone()
    }
}
impl ControlArg {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ControlFlow {
    pub branches: Vec<Continuation>,
}
impl From<&ControlFlow> for ControlFlow {
    fn from(value: &ControlFlow) -> Self {
        value.clone()
    }
}
impl ControlFlow {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DebugstrArg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
impl From<&DebugstrArg> for DebugstrArg {
    fn from(value: &DebugstrArg) -> Self {
        value.clone()
    }
}
impl DebugstrArg {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DeltaArg {
    pub arg: Box<Arg>,

    pub delta: i64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
impl From<&DeltaArg> for DeltaArg {
    fn from(value: &DeltaArg) -> Self {
        value.clone()
    }
}
impl DeltaArg {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Description {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs_links: Vec<DocsLink>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<Example>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exit_codes: Vec<ExitCode>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gas: Vec<GasConsumptionEntry>,

    pub long: String,

    pub operands: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other_implementations: Vec<OtherImplementation>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_instructions: Vec<String>,

    pub short: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}
impl From<&Description> for Description {
    fn from(value: &Description) -> Self {
        value.clone()
    }
}
impl Description {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DocsLink {
    pub name: String,

    pub url: String,
}
impl From<&DocsLink> for DocsLink {
    fn from(value: &DocsLink) -> Self {
        value.clone()
    }
}
impl DocsLink {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Example {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,

    pub instructions: Vec<ExampleInstruction>,
    pub stack: ExampleStack,
}
impl From<&Example> for Example {
    fn from(value: &Example) -> Self {
        value.clone()
    }
}
impl Example {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ExampleInstruction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,

    pub instruction: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_main: Option<bool>,
}
impl From<&ExampleInstruction> for ExampleInstruction {
    fn from(value: &ExampleInstruction) -> Self {
        value.clone()
    }
}
impl ExampleInstruction {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ExampleStack {
    pub input: Vec<String>,

    pub output: Vec<String>,
}
impl From<&ExampleStack> for ExampleStack {
    fn from(value: &ExampleStack) -> Self {
        value.clone()
    }
}
impl ExampleStack {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ExitCode {
    pub condition: String,

    pub errno: String,
}
impl From<&ExitCode> for ExitCode {
    fn from(value: &ExitCode) -> Self {
        value.clone()
    }
}
impl ExitCode {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ExoticCellArg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
impl From<&ExoticCellArg> for ExoticCellArg {
    fn from(value: &ExoticCellArg) -> Self {
        value.clone()
    }
}
impl ExoticCellArg {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum FiftArgument {
    Integer(i64),
    String(String),
}
impl From<&Self> for FiftArgument {
    fn from(value: &FiftArgument) -> Self {
        value.clone()
    }
}
impl std::fmt::Display for FiftArgument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Integer(x) => x.fmt(f),
            Self::String(x) => x.fmt(f),
        }
    }
}
impl From<i64> for FiftArgument {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FiftInstruction {
    pub actual_name: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<FiftArgument>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    pub name: String,
}
impl From<&FiftInstruction> for FiftInstruction {
    fn from(value: &FiftInstruction) -> Self {
        value.clone()
    }
}
impl FiftInstruction {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct GasConsumptionEntry {
    pub description: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,

    pub value: i64,
}
impl From<&GasConsumptionEntry> for GasConsumptionEntry {
    fn from(value: &GasConsumptionEntry) -> Self {
        value.clone()
    }
}
impl GasConsumptionEntry {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ImplementationInfo {
    pub commit_hash: String,

    pub file_path: String,

    pub function_name: String,

    pub line_number: i64,
}
impl From<&ImplementationInfo> for ImplementationInfo {
    fn from(value: &ImplementationInfo) -> Self {
        value.clone()
    }
}
impl ImplementationInfo {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct InlineCodeSliceArg {
    pub bits: Box<Arg>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
impl From<&InlineCodeSliceArg> for InlineCodeSliceArg {
    fn from(value: &InlineCodeSliceArg) -> Self {
        value.clone()
    }
}
impl InlineCodeSliceArg {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct InlineDictArg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
impl From<&InlineDictArg> for InlineDictArg {
    fn from(value: &InlineDictArg) -> Self {
        value.clone()
    }
}
impl InlineDictArg {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SpecInstruction {
    pub category: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_flow: Option<ControlFlow>,
    pub description: Description,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation: Option<ImplementationInfo>,
    pub layout: Layout,

    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<InstructionSignature>,

    pub sub_category: String,
}
impl From<&SpecInstruction> for SpecInstruction {
    fn from(value: &SpecInstruction) -> Self {
        value.clone()
    }
}
impl SpecInstruction {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct InstructionInputs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registers: Option<RegisterValues>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<StackValues>,
}
impl From<&InstructionInputs> for InstructionInputs {
    fn from(value: &InstructionInputs) -> Self {
        value.clone()
    }
}
impl InstructionInputs {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct InstructionOutputs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registers: Option<RegisterValues>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<StackValues>,
}
impl From<&InstructionOutputs> for InstructionOutputs {
    fn from(value: &InstructionOutputs) -> Self {
        value.clone()
    }
}
impl InstructionOutputs {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct InstructionSignature {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<InstructionInputs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outputs: Option<InstructionOutputs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack_string: Option<String>,
}
impl From<&InstructionSignature> for InstructionSignature {
    fn from(value: &InstructionSignature) -> Self {
        value.clone()
    }
}
impl InstructionSignature {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct IntArg {
    pub len: i64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    pub range: ArgRange,
}
impl From<&IntArg> for IntArg {
    fn from(value: &IntArg) -> Self {
        value.clone()
    }
}
impl IntArg {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct LargeIntArg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    pub range: ArgRange,
}
impl From<&LargeIntArg> for LargeIntArg {
    fn from(value: &LargeIntArg) -> Self {
        value.clone()
    }
}
impl LargeIntArg {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub args: Args,
    #[serde(rename = "checkLen")]
    pub check_len: i64,
    pub exec: String,

    pub kind: LayoutKind,

    pub max: i64,

    pub min: i64,

    pub prefix: i64,

    pub prefix_str: String,
    #[serde(rename = "skipLen")]
    pub skip_len: i64,

    pub tlb: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<i64>,
}
impl From<&Layout> for Layout {
    fn from(value: &Layout) -> Self {
        value.clone()
    }
}
impl Layout {}

#[derive(
    serde::Deserialize, serde::Serialize, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
pub enum LayoutKind {
    #[serde(rename = "ext")]
    Ext,
    #[serde(rename = "ext-range")]
    ExtRange,
    #[serde(rename = "fixed")]
    Fixed,
    #[serde(rename = "fixed-range")]
    FixedRange,
    #[serde(rename = "simple")]
    Simple,
}
impl From<&Self> for LayoutKind {
    fn from(value: &LayoutKind) -> Self {
        value.clone()
    }
}
impl std::fmt::Display for LayoutKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Ext => f.write_str("ext"),
            Self::ExtRange => f.write_str("ext-range"),
            Self::Fixed => f.write_str("fixed"),
            Self::FixedRange => f.write_str("fixed-range"),
            Self::Simple => f.write_str("simple"),
        }
    }
}
impl std::str::FromStr for LayoutKind {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> Result<Self, error::ConversionError> {
        match value {
            "ext" => Ok(Self::Ext),
            "ext-range" => Ok(Self::ExtRange),
            "fixed" => Ok(Self::Fixed),
            "fixed-range" => Ok(Self::FixedRange),
            "simple" => Ok(Self::Simple),
            _ => Err("invalid value".into()),
        }
    }
}
impl TryFrom<&str> for LayoutKind {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl TryFrom<&String> for LayoutKind {
    type Error = error::ConversionError;
    fn try_from(value: &String) -> Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl TryFrom<String> for LayoutKind {
    type Error = error::ConversionError;
    fn try_from(value: String) -> Result<Self, error::ConversionError> {
        value.parse()
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct MatchArm {
    pub stack: StackValues,
    pub value: ArmValue,
}
impl From<&MatchArm> for MatchArm {
    fn from(value: &MatchArm) -> Self {
        value.clone()
    }
}
impl MatchArm {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct MinusOneArg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
impl From<&MinusOneArg> for MinusOneArg {
    fn from(value: &MinusOneArg) -> Self {
        value.clone()
    }
}
impl MinusOneArg {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Mutation {
    pub length: MutationLength,
}
impl From<&Mutation> for Mutation {
    fn from(value: &Mutation) -> Self {
        value.clone()
    }
}
impl Mutation {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct MutationLength {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_arg: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack_amount_arg: Option<i64>,
}
impl From<&MutationLength> for MutationLength {
    fn from(value: &MutationLength) -> Self {
        value.clone()
    }
}
impl MutationLength {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct OtherImplementation {
    pub exact: bool,

    pub instructions: Vec<String>,
}
impl From<&OtherImplementation> for OtherImplementation {
    fn from(value: &OtherImplementation) -> Self {
        value.clone()
    }
}
impl OtherImplementation {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PlduzArg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    pub range: ArgRange,
}
impl From<&PlduzArg> for PlduzArg {
    fn from(value: &PlduzArg) -> Self {
        value.clone()
    }
}
impl PlduzArg {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PossibleValueRange {
    pub max: f64,
    pub min: f64,
}
impl From<&PossibleValueRange> for PossibleValueRange {
    fn from(value: &PossibleValueRange) -> Self {
        value.clone()
    }
}
impl PossibleValueRange {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct PossibleValueTypes(pub Vec<PossibleValueTypesItem>);
impl std::ops::Deref for PossibleValueTypes {
    type Target = Vec<PossibleValueTypesItem>;
    fn deref(&self) -> &Vec<PossibleValueTypesItem> {
        &self.0
    }
}
impl From<PossibleValueTypes> for Vec<PossibleValueTypesItem> {
    fn from(value: PossibleValueTypes) -> Self {
        value.0
    }
}
impl From<&PossibleValueTypes> for PossibleValueTypes {
    fn from(value: &PossibleValueTypes) -> Self {
        value.clone()
    }
}
impl From<Vec<PossibleValueTypesItem>> for PossibleValueTypes {
    fn from(value: Vec<PossibleValueTypesItem>) -> Self {
        Self(value)
    }
}

#[derive(
    serde::Deserialize, serde::Serialize, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
pub enum PossibleValueTypesItem {
    Int,
    Bool,
    Cell,
    Builder,
    Slice,
    Tuple,
    Continuation,
    Null,
    Any,
}
impl From<&Self> for PossibleValueTypesItem {
    fn from(value: &PossibleValueTypesItem) -> Self {
        value.clone()
    }
}
impl std::fmt::Display for PossibleValueTypesItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Int => f.write_str("Int"),
            Self::Bool => f.write_str("Bool"),
            Self::Cell => f.write_str("Cell"),
            Self::Builder => f.write_str("Builder"),
            Self::Slice => f.write_str("Slice"),
            Self::Tuple => f.write_str("Tuple"),
            Self::Continuation => f.write_str("Continuation"),
            Self::Null => f.write_str("Null"),
            Self::Any => f.write_str("Any"),
        }
    }
}
impl std::str::FromStr for PossibleValueTypesItem {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> Result<Self, error::ConversionError> {
        match value {
            "Int" => Ok(Self::Int),
            "Bool" => Ok(Self::Bool),
            "Cell" => Ok(Self::Cell),
            "Builder" => Ok(Self::Builder),
            "Slice" => Ok(Self::Slice),
            "Tuple" => Ok(Self::Tuple),
            "Continuation" => Ok(Self::Continuation),
            "Null" => Ok(Self::Null),
            "Any" => Ok(Self::Any),
            _ => Err("invalid value".into()),
        }
    }
}
impl TryFrom<&str> for PossibleValueTypesItem {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl TryFrom<&String> for PossibleValueTypesItem {
    type Error = error::ConversionError;
    fn try_from(value: &String) -> Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl TryFrom<String> for PossibleValueTypesItem {
    type Error = error::ConversionError;
    fn try_from(value: String) -> Result<Self, error::ConversionError> {
        value.parse()
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct RefCodeSliceArg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
impl From<&RefCodeSliceArg> for RefCodeSliceArg {
    fn from(value: &RefCodeSliceArg) -> Self {
        value.clone()
    }
}
impl RefCodeSliceArg {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Register {
    #[serde(rename = "constant")]
    Constant { index: i64 },
    #[serde(rename = "variable")]
    Variable { var_name: VariableName },
    #[serde(rename = "special")]
    Special { name: RegisterName },
}
impl From<&Self> for Register {
    fn from(value: &Register) -> Self {
        value.clone()
    }
}

#[derive(
    serde::Deserialize, serde::Serialize, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
pub enum RegisterName {
    #[serde(rename = "gas")]
    Gas,
    #[serde(rename = "cstate")]
    Cstate,
    #[serde(rename = "r")]
    R,
}
impl From<&Self> for RegisterName {
    fn from(value: &RegisterName) -> Self {
        value.clone()
    }
}
impl std::fmt::Display for RegisterName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Gas => f.write_str("gas"),
            Self::Cstate => f.write_str("cstate"),
            Self::R => f.write_str("r"),
        }
    }
}
impl std::str::FromStr for RegisterName {
    type Err = error::ConversionError;
    fn from_str(value: &str) -> Result<Self, error::ConversionError> {
        match value {
            "gas" => Ok(Self::Gas),
            "cstate" => Ok(Self::Cstate),
            "r" => Ok(Self::R),
            _ => Err("invalid value".into()),
        }
    }
}
impl TryFrom<&str> for RegisterName {
    type Error = error::ConversionError;
    fn try_from(value: &str) -> Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl TryFrom<&String> for RegisterName {
    type Error = error::ConversionError;
    fn try_from(value: &String) -> Result<Self, error::ConversionError> {
        value.parse()
    }
}
impl TryFrom<String> for RegisterName {
    type Error = error::ConversionError;
    fn try_from(value: String) -> Result<Self, error::ConversionError> {
        value.parse()
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct RegisterValues(pub Vec<Register>);
impl std::ops::Deref for RegisterValues {
    type Target = Vec<Register>;
    fn deref(&self) -> &Vec<Register> {
        &self.0
    }
}
impl From<RegisterValues> for Vec<Register> {
    fn from(value: RegisterValues) -> Self {
        value.0
    }
}
impl From<&RegisterValues> for RegisterValues {
    fn from(value: &RegisterValues) -> Self {
        value.clone()
    }
}
impl From<Vec<Register>> for RegisterValues {
    fn from(value: Vec<Register>) -> Self {
        Self(value)
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct S1Arg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
impl From<&S1Arg> for S1Arg {
    fn from(value: &S1Arg) -> Self {
        value.clone()
    }
}
impl S1Arg {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct Savelist {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c0: Option<Box<Continuation>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c1: Option<Box<Continuation>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c2: Option<Box<Continuation>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c3: Option<Box<Continuation>>,
}
impl From<&Savelist> for Savelist {
    fn from(value: &Savelist) -> Self {
        value.clone()
    }
}
impl Savelist {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SetcpArg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    pub range: ArgRange,
}
impl From<&SetcpArg> for SetcpArg {
    fn from(value: &SetcpArg) -> Self {
        value.clone()
    }
}
impl SetcpArg {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SliceArg {
    pub bits: Box<Arg>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    pub pad: i64,

    pub refs: Box<Arg>,
}
impl From<&SliceArg> for SliceArg {
    fn from(value: &SliceArg) -> Self {
        value.clone()
    }
}
impl SliceArg {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Specification {
    pub fift_instructions: Vec<FiftInstruction>,
    pub instructions: Vec<SpecInstruction>,
    #[serde(rename = "$schema")]
    pub schema: String,
    pub version: String,
}
impl From<&Specification> for Specification {
    fn from(value: &Specification) -> Self {
        value.clone()
    }
}
impl Specification {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct StackArg {
    pub len: i64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    pub range: ArgRange,
}
impl From<&StackArg> for StackArg {
    fn from(value: &StackArg) -> Self {
        value.clone()
    }
}
impl StackArg {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum StackEntry {
    #[serde(rename = "simple")]
    Simple {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        mutations: Vec<Mutation>,
        name: VariableName,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        presentation: Option<String>,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        range: Option<PossibleValueRange>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value_types: Option<PossibleValueTypes>,
    },
    #[serde(rename = "const")]
    Const {
        value: ConstantValue,
        value_type: ConstantType,
    },
    #[serde(rename = "conditional")]
    Conditional {
        #[serde(rename = "else", default, skip_serializing_if = "Option::is_none")]
        else_: Option<StackValues>,
        #[serde(rename = "match")]
        match_: Vec<MatchArm>,
        name: VariableName1,
    },
    #[serde(rename = "array")]
    Array {
        array_entry: ArraySingleEntryDefinition,
        length_var: VariableName2,
        name: VariableName,
    },
}
impl From<&Self> for StackEntry {
    fn from(value: &StackEntry) -> Self {
        value.clone()
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct StackValues(pub Vec<StackEntry>);
impl std::ops::Deref for StackValues {
    type Target = Vec<StackEntry>;
    fn deref(&self) -> &Vec<StackEntry> {
        &self.0
    }
}
impl From<StackValues> for Vec<StackEntry> {
    fn from(value: StackValues) -> Self {
        value.0
    }
}
impl From<&StackValues> for StackValues {
    fn from(value: &StackValues) -> Self {
        value.clone()
    }
}
impl From<Vec<StackEntry>> for StackValues {
    fn from(value: Vec<StackEntry>) -> Self {
        Self(value)
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TinyIntArg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    pub range: ArgRange,
}
impl From<&TinyIntArg> for TinyIntArg {
    fn from(value: &TinyIntArg) -> Self {
        value.clone()
    }
}
impl TinyIntArg {}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct UintArg {
    pub len: i64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    pub range: ArgRange,
}
impl From<&UintArg> for UintArg {
    fn from(value: &UintArg) -> Self {
        value.clone()
    }
}
impl UintArg {}

#[derive(
    serde::Deserialize, serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
#[serde(transparent)]
pub struct VariableName(pub String);
impl std::ops::Deref for VariableName {
    type Target = String;
    fn deref(&self) -> &String {
        &self.0
    }
}
impl From<VariableName> for String {
    fn from(value: VariableName) -> Self {
        value.0
    }
}
impl From<&VariableName> for VariableName {
    fn from(value: &VariableName) -> Self {
        value.clone()
    }
}
impl From<String> for VariableName {
    fn from(value: String) -> Self {
        Self(value)
    }
}
impl std::str::FromStr for VariableName {
    type Err = Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
impl std::fmt::Display for VariableName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(
    serde::Deserialize, serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
#[serde(transparent)]
pub struct VariableName1(pub String);
impl std::ops::Deref for VariableName1 {
    type Target = String;
    fn deref(&self) -> &String {
        &self.0
    }
}
impl From<VariableName1> for String {
    fn from(value: VariableName1) -> Self {
        value.0
    }
}
impl From<&VariableName1> for VariableName1 {
    fn from(value: &VariableName1) -> Self {
        value.clone()
    }
}
impl From<String> for VariableName1 {
    fn from(value: String) -> Self {
        Self(value)
    }
}
impl std::str::FromStr for VariableName1 {
    type Err = Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
impl std::fmt::Display for VariableName1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(
    serde::Deserialize, serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd,
)]
#[serde(transparent)]
pub struct VariableName2(pub String);
impl std::ops::Deref for VariableName2 {
    type Target = String;
    fn deref(&self) -> &String {
        &self.0
    }
}
impl From<VariableName2> for String {
    fn from(value: VariableName2) -> Self {
        value.0
    }
}
impl From<&VariableName2> for VariableName2 {
    fn from(value: &VariableName2) -> Self {
        value.clone()
    }
}
impl From<String> for VariableName2 {
    fn from(value: String) -> Self {
        Self(value)
    }
}
impl std::str::FromStr for VariableName2 {
    type Err = Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
impl std::fmt::Display for VariableName2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
