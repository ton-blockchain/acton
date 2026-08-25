use crate::CodegenError;
use crate::declarations::DeclarationEmitter;
use crate::wrappers::WrapperEmitter;
use proc_macro2::TokenStream;
use quote::quote;
use tolk_source_map::abi::ContractABI;

/// Coordinates the same independent stages as the upstream generator.
pub(crate) struct RustBackend<'abi> {
    abi: &'abi ContractABI,
}

impl<'abi> RustBackend<'abi> {
    pub(crate) const fn new(abi: &'abi ContractABI) -> Self {
        Self { abi }
    }

    pub(crate) fn emit(&self) -> Result<TokenStream, CodegenError> {
        let schema_version = &self.abi.abi_schema_version;
        let code_boc64 = &self.abi.code_boc64;
        let contract_name = &self.abi.contract_name;
        let compiler_name = &self.abi.compiler_name;
        let compiler_version = &self.abi.compiler_version;
        let declarations = DeclarationEmitter::new(self.abi).emit()?;
        let wrapper = WrapperEmitter::new(self.abi).emit()?;

        Ok(quote! {
            pub const ABI_SCHEMA_VERSION: &str = #schema_version;
            pub const CODE_BOC64: &str = #code_boc64;
            pub const CONTRACT_NAME: &str = #contract_name;
            pub const COMPILER_NAME: &str = #compiler_name;
            pub const COMPILER_VERSION: &str = #compiler_version;

            #declarations
            #wrapper
        })
    }
}
