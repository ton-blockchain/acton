use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DataStruct, DeriveInput, Fields, parse_macro_input};

#[proc_macro_derive(TupleSerialize)]
pub fn tuple_serialize_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let fields = match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(named_fields),
            ..
        }) => &named_fields.named,
        _ => {
            return syn::Error::new_spanned(
                &input,
                "TupleSerialize can only be derived for structs with named fields",
            )
            .to_compile_error()
            .into();
        }
    };

    let mut field_serializations = Vec::new();

    for field in fields {
        let field_name = &field.ident;

        let mut flatten = false;
        for attr in &field.attrs {
            if attr.path().is_ident("tuple") {
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("flatten") {
                        flatten = true;
                        Ok(())
                    } else {
                        Err(meta.error("unknown tuple attribute"))
                    }
                });
            }
        }

        if flatten {
            field_serializations.push(quote! {
                tvmffi::to_stack::ToStack::to_tuple(&self.#field_name, &mut tuple, _options)?
            });
        } else {
            field_serializations.push(quote! {
                tuple.push(tvmffi::to_stack::ToStack::to_item(&self.#field_name)?)
            });
        }
    }

    let expanded = quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            pub fn to_tuple(&self, _options: tvmffi::to_stack::SerializationOptions) -> Result<tvmffi::stack::Tuple, tvmffi::to_stack::SerializationError> {
                let mut tuple = tvmffi::stack::Tuple(vec![]);
                #(#field_serializations;)*
                Ok(tuple)
            }
        }

        impl #impl_generics tvmffi::to_stack::ToStack for #name #ty_generics #where_clause {
            fn to_item(&self) -> Result<tvmffi::stack::TupleItem, tvmffi::to_stack::SerializationError> {
                Ok(tvmffi::stack::TupleItem::Tuple(self.to_tuple(Default::default())?))
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(TupleDeserialize, attributes(tuple))]
pub fn tuple_deserialize_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("TupleDeserialize only supports structs with named fields"),
        },
        _ => panic!("TupleDeserialize only supports structs"),
    };

    let mut field_deserializations = Vec::new();
    let mut field_counts = Vec::new();

    for field in fields {
        let field_name = &field.ident;
        let field_type = &field.ty;

        let mut flatten = false;
        for attr in &field.attrs {
            if attr.path().is_ident("tuple") {
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("flatten") {
                        flatten = true;
                        Ok(())
                    } else {
                        Err(meta.error("unknown tuple attribute"))
                    }
                });
            }
        }

        if flatten {
            field_deserializations.push(quote! {
                #field_name: <#field_type as tvmffi::from_stack::FromStack>::from_tuple(tuple, offset, options)?
            });
            field_counts.push(quote! {
                <#field_type as tvmffi::from_stack::FromStack>::FIELD_COUNT
            });
        } else {
            field_deserializations.push(quote! {
                #field_name: {
                    let item = tuple.get(*offset).cloned().unwrap_or(tvmffi::stack::TupleItem::Null);
                    *offset += 1;
                    <#field_type as tvmffi::from_stack::FromStack>::from_item(item)?
                }
            });
            field_counts.push(quote! { 1 });
        }
    }

    let expanded = quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            pub const FIELD_COUNT: usize = 0 #(+ #field_counts)*;

            pub fn from_tuple(tuple: &tvmffi::stack::Tuple, options: tvmffi::from_stack::DeserializationOptions) -> Result<Self, tvmffi::from_stack::ArgError> {
                let mut offset = 0;
                Self::from_tuple_at(tuple, &mut offset, options)
            }

            pub fn from_tuple_at(tuple: &tvmffi::stack::Tuple, offset: &mut usize, options: tvmffi::from_stack::DeserializationOptions) -> Result<Self, tvmffi::from_stack::ArgError> {
                let initial_offset = *offset;
                let res = Ok(Self {
                    #(#field_deserializations),*
                });
                let len = *offset - initial_offset;
                let expected = Self::FIELD_COUNT;

                if options.allow_extra && options.allow_missing {
                    // no check
                } else if options.allow_extra {
                    if len < expected {
                        return Err(tvmffi::from_stack::ArgError::MissingElements { expected, actual: len });
                    }
                } else if options.allow_missing {
                    if len > expected {
                        return Err(tvmffi::from_stack::ArgError::ExtraElements { expected, actual: len });
                    }
                } else {
                    if len != expected {
                        if len < expected {
                            return Err(tvmffi::from_stack::ArgError::MissingElements { expected, actual: len });
                        } else {
                            return Err(tvmffi::from_stack::ArgError::ExtraElements { expected, actual: len });
                        }
                    }
                }
                res
            }
        }

        impl #impl_generics tvmffi::from_stack::FromStack for #name #ty_generics #where_clause {
            const FIELD_COUNT: usize = Self::FIELD_COUNT;

            fn from_item(item: tvmffi::stack::TupleItem) -> Result<Self, tvmffi::from_stack::ArgError> {
                match item {
                    tvmffi::stack::TupleItem::Tuple(tuple) => Self::from_tuple(&tuple, Default::default()),
                    tvmffi::stack::TupleItem::TypedTuple { inner, .. } => Self::from_tuple(&inner, Default::default()),
                    _ => Err(tvmffi::from_stack::ArgError::TypeMismatch { expected: "Tuple" }),
                }
            }

            fn from_tuple(tuple: &tvmffi::stack::Tuple, offset: &mut usize, options: tvmffi::from_stack::DeserializationOptions) -> Result<Self, tvmffi::from_stack::ArgError> {
                Self::from_tuple_at(tuple, offset, options)
            }
        }
    };

    TokenStream::from(expanded)
}
