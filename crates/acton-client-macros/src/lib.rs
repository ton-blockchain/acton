use acton_client_codegen::{GenerateOptions, generate_tokens_with_options};
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use syn::parse::{Parse, ParseStream};
use syn::{Error, ItemMod, LitStr, Result, Token, parse_macro_input};

struct ContractArgs {
    abi: LitStr,
}

impl Parse for ContractArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let key: Ident = input.parse()?;
        if key != "abi" {
            return Err(Error::new(
                key.span(),
                "expected `abi = \"path/to/contract.abi.json\"`",
            ));
        }
        input.parse::<Token![=]>()?;
        let abi = input.parse()?;
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
        if !input.is_empty() {
            return Err(input.error("unexpected contract macro argument"));
        }
        Ok(Self { abi })
    }
}

#[proc_macro_attribute]
pub fn contract(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as ContractArgs);
    let module = parse_macro_input!(item as ItemMod);

    expand_contract(&args, module)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn expand_contract(args: &ContractArgs, module: ItemMod) -> Result<TokenStream2> {
    let abi_path = resolve_abi_path(&args.abi)?;
    let source = fs::read_to_string(&abi_path).map_err(|error| {
        Error::new(
            args.abi.span(),
            format!("failed to read ABI `{}`: {error}", abi_path.display()),
        )
    })?;
    let generated = generate_tokens_with_options(
        &source,
        GenerateOptions {
            embed_abi_json: false,
        },
    )
    .map_err(|error| Error::new(args.abi.span(), error))?;

    let Some((_, original_items)) = module.content else {
        return Err(Error::new_spanned(
            module,
            "#[acton_client::contract] requires an inline module",
        ));
    };
    let abi_path = LitStr::new(path_as_str(&abi_path)?, args.abi.span());
    let attrs = module.attrs;
    let visibility = module.vis;
    let module_name = module.ident;

    Ok(quote! {
        #(#attrs)*
        #visibility mod #module_name {
            #(#original_items)*

            pub const ABI_JSON: &str = include_str!(#abi_path);
            #generated
        }
    })
}

fn resolve_abi_path(path: &LitStr) -> Result<PathBuf> {
    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR").ok_or_else(|| {
        Error::new(
            path.span(),
            "CARGO_MANIFEST_DIR is unavailable while expanding contract ABI",
        )
    })?;
    let path = Path::new(&manifest_dir).join(path.value());
    dunce::canonicalize(&path).map_err(|error| {
        Error::new(
            Span::call_site(),
            format!("failed to resolve ABI `{}`: {error}", path.display()),
        )
    })
}

fn path_as_str(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| Error::new(Span::call_site(), "ABI path is not valid UTF-8"))
}
