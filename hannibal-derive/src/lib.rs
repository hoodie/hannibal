extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{parse_macro_input, DeriveInput};

/// Implement an hannibal message type.
///
/// The return value type defaults to (), and you can specify the type with the result parameter.
///
/// # Examples
///
/// ```ignore
/// #[message(result = i32)]
/// struct TestMessage(i32);
/// ```
#[proc_macro_attribute]
pub fn message(args: TokenStream, input: TokenStream) -> TokenStream {
    // let mut res: Option<syn::ExprPath> = None;
    let mut res: Option<syn::Type> = None;

    let args_parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("result") {
            res = Some(meta.value()?.parse()?);
            Ok(())
        } else {
            Ok(())
        }
    });

    parse_macro_input!(args with args_parser);
    let result_type = if let Some(ty) = res {
        quote! { #ty }
    } else {
        quote! { () }
    };

    let input = parse_macro_input!(input as DeriveInput);
    let ident = &input.ident;
    let expanded = quote! {
        #input
        impl hannibal::Message for #ident {
            type Result = #result_type;
            const TYPE_NAME: &'static str = stringify!(#ident);
        }
    };
    expanded.into()
}

/// Implement an hannibal main function.
///
/// Wait for all actors to exit.
#[proc_macro_attribute]
pub fn main(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::ItemFn);

    if &*input.sig.ident.to_string() != "main" {
        return TokenStream::from(quote_spanned! { input.sig.ident.span() =>
            compile_error!("only the main function can be tagged with #[hannibal::main]"),
        });
    }

    if input.sig.asyncness.is_none() {
        return TokenStream::from(quote_spanned! { input.span() =>
            compile_error!("the async keyword is missing from the function declaration"),
        });
    }

    input.sig.ident = Ident::new("__main", Span::call_site());
    let ret = &input.sig.output;

    let expanded = quote! {
        #input

        fn main() #ret {
            hannibal::block_on(__main())
        }
    };

    expanded.into()
}
