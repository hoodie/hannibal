extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{DeriveInput, Ident, parse_macro_input};

#[proc_macro_attribute]
pub fn main(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut input: syn::ItemFn = syn::parse_macro_input!(input);

    input.sig.ident = Ident::new("original_main", Span::call_site());
    let return_type = &input.sig.output;

    let generated = quote! {
        #input

        fn main() #return_type {
            hannibal::runtime::block_on(original_main())
        }
    };

    generated.into()
}

#[proc_macro_attribute]
pub fn test(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut input: syn::ItemFn = syn::parse_macro_input!(input);

    let original_name = &input.sig.ident.clone();
    input.sig.ident = Ident::new("inner_test", Span::call_site());
    let return_type = &input.sig.output;

    let generated = quote! {
        #input

        #[test]
        async fn #original_name() #return_type {
            hannibal::runtime::block_on(inner_test())
        }
    };

    generated.into()
}

#[proc_macro_attribute]
pub fn message(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut response: Option<syn::Type> = None;

    let response_parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("response") {
            response = Some(meta.value()?.parse()?);
            Ok(())
        } else {
            Ok(())
        }
    });

    parse_macro_input!(args with response_parser);
    let response_type = if let Some(ty) = response {
        quote! { #ty }
    } else {
        quote! { () }
    };

    let ast = syn::parse::<DeriveInput>(input).unwrap();
    let ident = &ast.ident;
    let generated = quote! {
        #ast
        impl ::hannibal::Message for #ident {
            type Response = #response_type;
        }
    };
    generated.into()
}

#[proc_macro_derive(Message)]
pub fn derive_message(input: TokenStream) -> TokenStream {
    let ast = syn::parse::<DeriveInput>(input).unwrap();

    let name = &ast.ident;
    let generated = quote! {
        impl ::hannibal::Message for #name {
            type Response = ();
        }
    };
    generated.into()
}

#[proc_macro_derive(RestartableActor)]
pub fn derive_restartable_actor(input: TokenStream) -> TokenStream {
    let ast = syn::parse::<DeriveInput>(input).unwrap();

    let name = &ast.ident;
    let generated = quote! {
        impl ::hannibal::RestartableActor for #name {
        }
    };
    generated.into()
}
#[proc_macro_derive(Actor)]
pub fn derive_actor(input: TokenStream) -> TokenStream {
    let ast = syn::parse::<DeriveInput>(input).unwrap();

    let name = &ast.ident;
    let generated = quote! {
        impl ::hannibal::Actor for #name {
        }
    };
    generated.into()
}

#[proc_macro_derive(Service)]
pub fn derive_service(input: TokenStream) -> TokenStream {
    let ast = syn::parse::<DeriveInput>(input).unwrap();

    let name = &ast.ident;
    let generated = quote! {
        impl ::hannibal::Service for #name {
        }
    };
    generated.into()
}
