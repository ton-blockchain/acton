use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, parse_macro_input, parse_quote};

#[proc_macro_derive(FromStackTuple)]
pub fn derive_from_stack_tuple(item: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(item);

    expand_from_stack_tuple(input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn expand_from_stack_tuple(mut input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let struct_name = input.ident;
    let fields = match input.data {
        Data::Struct(data) => data.fields,
        _ => {
            return Err(Error::new_spanned(
                struct_name,
                "FromStackTuple can only be derived for structs",
            ));
        }
    };

    for field in &fields {
        let ty = &field.ty;
        input
            .generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(#ty: ::tvm_ffi::from_stack::FromStack));
    }

    let field_count = fields.len();
    let build = match fields {
        Fields::Named(fields) => {
            let field_inits = fields
                .named
                .iter()
                .map(|field| {
                    let field_name = field.ident.as_ref().ok_or_else(|| {
                        Error::new_spanned(field, "named field must have an identifier")
                    })?;

                    Ok(quote! {
                    #field_name: ::tvm_ffi::from_stack::FromStack::from_item(
                        __items
                            .next()
                            .ok_or(::tvm_ffi::from_stack::ArgError::StackUnderflow)?
                    )?
                    })
                })
                .collect::<syn::Result<Vec<_>>>()?;

            quote! {
                Self {
                    #(#field_inits),*
                }
            }
        }
        Fields::Unnamed(fields) => {
            let field_inits = fields.unnamed.iter().map(|_| {
                quote! {
                    ::tvm_ffi::from_stack::FromStack::from_item(
                        __items
                            .next()
                            .ok_or(::tvm_ffi::from_stack::ArgError::StackUnderflow)?
                    )?
                }
            });

            quote! {
                Self(
                    #(#field_inits),*
                )
            }
        }
        Fields::Unit => quote!(Self),
    };

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics ::tvm_ffi::from_stack::FromStackTuple
            for #struct_name #ty_generics
            #where_clause
        {
            fn from_tuple(
                __tuple: ::tvm_ffi::stack::Tuple,
            ) -> Result<Self, ::tvm_ffi::from_stack::ArgError> {
                let __actual_len = __tuple.len();
                if __actual_len != #field_count {
                    return Err(::tvm_ffi::from_stack::ArgError::TupleLengthMismatch {
                        expected: #field_count,
                        actual: __actual_len,
                    });
                }

                let mut __items = __tuple.0.into_iter();
                Ok(#build)
            }
        }
    })
}
