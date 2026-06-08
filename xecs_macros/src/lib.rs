use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{
    DeriveInput, Ident, LitInt, Result, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token::Comma,
};

use crate::component::Components;

mod component;

struct AllTuples {
    macro_ident: Ident,
    start: usize,
    end: usize,
}

impl Parse for AllTuples {
    fn parse(input: ParseStream) -> Result<Self> {
        let macro_ident = input.parse::<Ident>()?;
        input.parse::<Comma>()?;
        let start = input.parse::<LitInt>()?.base10_parse()?;
        input.parse::<Comma>()?;
        let end = input.parse::<LitInt>()?.base10_parse()?;

        Ok(AllTuples { macro_ident, start, end })
    }
}

#[proc_macro]
pub fn all_tuples(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as AllTuples);
    let len = 1 + input.end - input.start;
    let mut items = Vec::with_capacity(len);
    for i in 0..=len {
        items.push(format_ident!("P{}", i));
    }

    let macro_ident = &input.macro_ident;
    let invocations = (input.start..=input.end).map(|i| {
        let tuples = &items[..i];

        quote! {
            #macro_ident!(#(#tuples),*);
        }
    });

    quote! {
        #(
            #invocations
        )*
    }
    .into()
}

struct ParamItem {
    ident: Type,
    is_mut: bool,
    is_opt: bool,
}

impl Parse for ParamItem {
    fn parse(input: ParseStream) -> Result<Self> {
        let is_mut = input.peek(Token![mut]);

        if is_mut {
            input.parse::<Token![mut]>()?;
        }

        let ident = input.parse()?;

        let is_opt = input.peek(Token![?]);

        if is_opt {
            input.parse::<Token![?]>()?;
        }

        Ok(ParamItem { ident, is_mut, is_opt })
    }
}

impl ToTokens for ParamItem {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = &self.ident;

        let mut_ref = if self.is_mut {
            quote! { mut }
        } else {
            quote! {}
        };

        if self.is_opt {
            tokens.extend(quote! {core::option::Option<&#mut_ref #ident>})
        } else {
            tokens.extend(quote! {&#mut_ref #ident})
        }
    }
}

struct Params {
    items: Punctuated<ParamItem, Token![,]>,
}

impl Parse for Params {
    fn parse(input: ParseStream) -> Result<Self> {
        let items = input.parse_terminated(ParamItem::parse, Token![,])?;

        if items.is_empty() {
            return Err(input.error("expected at least one parameter"));
        }

        Ok(Self { items })
    }
}

impl ToTokens for Params {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let items = &self.items;

        tokens.extend(if items.len() > 1 {
            quote! {(#items)}
        } else {
            quote! { #items }
        });
    }
}

#[proc_macro]
pub fn params(input: TokenStream) -> TokenStream {
    let params = parse_macro_input!(input as Params);
    quote! { #params }.into()
}

#[proc_macro_derive(Component, attributes(component))]
pub fn component(input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as DeriveInput);
    impl_component(&item)
}

fn impl_component(input: &DeriveInput) -> TokenStream {
    let mut storage = quote! {xecs::storage::StorageType::Tables};

    for attr in &input.attrs {
        if attr.path().is_ident("component") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("storage") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;

                    storage = match lit.value().as_str() {
                        "sparse" => quote! {xecs::storage::StorageType::Sparse},
                        "tables" => quote! {xecs::storage::StorageType::Tables},
                        _ => quote! {compile_error!("xecs: invalid component storage type")},
                    };
                }

                Ok(())
            })
            .unwrap();
        }
    }

    let name = &input.ident;

    if !input.generics.params.is_empty() {
        return quote! {
            compile_error!("xecs: generic types are not supported for Component derive, use component! macro instead.");
        }
        .into();
    }

    if let syn::Data::Union(_) = &input.data {
        return quote! { compile_error!("xecs: union type not supported for components."); }.into();
    }

    quote! {
        impl xecs::component::TypedStaticId for #name {
            fn id() -> &'static xecs::component::StaticId<#name> {
                static COMP: std::sync::LazyLock<xecs::component::StaticId<#name>> =
                std::sync::LazyLock::new(||xecs::component::StaticId::new(stringify!(#name), #storage));
                &COMP
            }
        }
    }
    .into()
}

#[proc_macro]
pub fn components(input: TokenStream) -> TokenStream {
    let components = parse_macro_input!(input as Components);
    quote! { #components }.into()
}
