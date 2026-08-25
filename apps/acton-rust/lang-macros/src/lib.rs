use proc_macro::TokenStream;

fn marker(_args: TokenStream, item: TokenStream) -> TokenStream {
    item
}

fn prepend(attributes: &str, item: TokenStream) -> TokenStream {
    let mut output = attributes
        .parse::<TokenStream>()
        .expect("static Rust attributes must be valid tokens");
    output.extend(item);
    output
}

#[proc_macro_attribute]
pub fn contract(args: TokenStream, item: TokenStream) -> TokenStream {
    marker(args, item)
}

#[proc_macro_attribute]
pub fn storage(args: TokenStream, item: TokenStream) -> TokenStream {
    marker(args, item)
}

#[proc_macro_attribute]
pub fn message(args: TokenStream, item: TokenStream) -> TokenStream {
    marker(args, item)
}

#[proc_macro_attribute]
pub fn receive(args: TokenStream, item: TokenStream) -> TokenStream {
    let item = marker(args, item);
    prepend("#[allow(clippy::needless_pass_by_value)]", item)
}

#[proc_macro_attribute]
pub fn get(args: TokenStream, item: TokenStream) -> TokenStream {
    let item = marker(args, item);
    prepend("#[must_use] #[allow(clippy::needless_pass_by_value)]", item)
}
