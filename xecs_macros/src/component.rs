use quote::{ToTokens, quote};
use syn::{
    Ident, Result, Token, Type, Visibility, bracketed,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

struct ComponentDesc {
    vis: Visibility,
    name: Ident,
    storage: Option<Ident>,
    ty: Option<Type>,
}

impl Parse for ComponentDesc {
    fn parse(input: ParseStream) -> Result<Self> {
        let vis = input.parse()?;
        let name = input.parse()?;

        let (storage, ty) = if input.peek(Token![:]) {
            input.parse::<Token![:]>()?;

            let storage = if input.peek(Token![#]) {
                input.parse::<Token![#]>()?;
                let attr;
                bracketed!(attr in input);
                Some(attr.parse()?)
            } else {
                None
            };

            (storage, Some(input.parse()?))
        } else {
            (None, None)
        };

        Ok(ComponentDesc { vis, name, storage, ty })
    }
}

impl ToTokens for ComponentDesc {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let vis = &self.vis;
        let name = &self.name;

        let storage = match &self.storage {
            Some(storage) => {
                if storage == "tables" {
                    quote! {xecs::storage::StorageType::Tables}
                } else if storage == "sparse" {
                    quote! {xecs::storage::StorageType::Sparse}
                } else {
                    quote! {compile_error!("xecs: invalid component storage type")}
                }
            }
            None => quote! {xecs::storage::StorageType::Tables},
        };

        let ty = self.ty.as_ref().map_or(quote! {()}, |ty| quote! {#ty});

        tokens.extend(quote! {
            #[allow(non_upper_case_globals)]
            #vis static #name: std::sync::LazyLock<xecs::component::StaticId<#ty>> =
                std::sync::LazyLock::new(||xecs::component::StaticId::new(stringify!(#name), #storage));
        });
    }
}

pub struct Components {
    items: Punctuated<ComponentDesc, Token![,]>,
}

impl Parse for Components {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            items: input.parse_terminated(ComponentDesc::parse, Token![,])?,
        })
    }
}

impl ToTokens for Components {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let items = self.items.iter();
        tokens.extend(quote! {#(#items)*});
    }
}
