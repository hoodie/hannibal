extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

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
    let gen = quote! {
        #ast
        impl ::minibal::Message for #ident {
            type Response = #response_type;
        }
    };
    gen.into()
}

#[proc_macro_derive(Message)]
pub fn derive_message(input: TokenStream) -> TokenStream {
    let ast = syn::parse::<DeriveInput>(input).unwrap();

    let name = &ast.ident;
    let gen = quote! {
        impl ::minibal::Message for #name {
            type Response = ();
        }
    };
    gen.into()
}

#[proc_macro_derive(Actor)]
pub fn derive_actor(input: TokenStream) -> TokenStream {
    let ast = syn::parse::<DeriveInput>(input).unwrap();

    let name = &ast.ident;
    let gen = quote! {
        impl ::minibal::Actor for #name {
        }
    };
    gen.into()
}

#[proc_macro_derive(Service)]
pub fn derive_service(input: TokenStream) -> TokenStream {
    let ast = syn::parse::<DeriveInput>(input).unwrap();

    let name = &ast.ident;
    let gen = quote! {
        impl ::minibal::Service for #name {
        }
    };
    gen.into()
}
