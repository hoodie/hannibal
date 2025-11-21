use std::env::var;

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::Span;
use quote::quote;
use syn::{DeriveInput, Ident, parse_macro_input};

fn get_crate_path() -> proc_macro2::TokenStream {
    match (crate_name("hannibal"), var("CARGO_CRATE_NAME").as_deref()) {
        (Ok(FoundCrate::Itself), Ok("hannibal")) => quote!(crate),
        (Ok(FoundCrate::Itself), _) => quote!(::hannibal),
        (Ok(FoundCrate::Name(_name)), _) => quote!(::hannibal),
        (Err(_), _) => quote!(::hannibal),
    }
}

#[proc_macro_attribute]
pub fn main(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut input: syn::ItemFn = syn::parse_macro_input!(input);

    input.sig.ident = Ident::new("original_main", Span::call_site());
    let return_type = &input.sig.output;
    let crate_path = get_crate_path();

    let generated = quote! {
        #input

        fn main() #return_type {
            #crate_path::runtime::block_on(original_main())
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
    let crate_path = get_crate_path();
    let generated = quote! {
        #ast
        impl #crate_path::Message for #ident {
            type Response = #response_type;
        }
    };
    generated.into()
}

#[proc_macro_derive(Message)]
pub fn derive_message(input: TokenStream) -> TokenStream {
    let ast = syn::parse::<DeriveInput>(input).unwrap();

    let name = &ast.ident;
    let crate_path = get_crate_path();
    let generated = quote! {
        impl #crate_path::Message for #name {
            type Response = ();
        }
    };
    generated.into()
}

#[proc_macro_derive(RestartableActor)]
pub fn derive_restartable_actor(input: TokenStream) -> TokenStream {
    let ast = syn::parse::<DeriveInput>(input).unwrap();

    let name = &ast.ident;
    let crate_path = get_crate_path();
    let generated = quote! {
        impl #crate_path::RestartableActor for #name {
        }
    };
    generated.into()
}

#[proc_macro_derive(Actor)]
pub fn derive_actor(input: TokenStream) -> TokenStream {
    let ast = syn::parse::<DeriveInput>(input).unwrap();

    let name = &ast.ident;
    let generics = &ast.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let crate_path = get_crate_path();
    let generated = quote! {
        impl #impl_generics #crate_path::Actor for #name #ty_generics #where_clause {
        }
    };
    generated.into()
}

#[proc_macro_derive(Service)]
pub fn derive_service(input: TokenStream) -> TokenStream {
    let ast = syn::parse::<DeriveInput>(input).unwrap();

    let name = &ast.ident;
    let crate_path = get_crate_path();
    let generated = quote! {
        impl #crate_path::Service for #name {
        }
    };
    generated.into()
}
