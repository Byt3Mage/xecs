use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{
    DeriveInput, Ident, LitInt, Result, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token::Comma,
};

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

        Ok(AllTuples {
            macro_ident,
            start,
            end,
        })
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

        Ok({
            ParamItem {
                ident,
                is_mut,
                is_opt,
            }
        })
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

#[proc_macro_derive(Component)]
pub fn component(input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as DeriveInput);
    impl_component(&item)
}

fn impl_component(ast: &DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let has_generics = !ast.generics.params.is_empty();

    let (is_generic, type_index) = if has_generics {
        (
            quote! { const IS_GENERIC: bool = true; },
            quote! { xecs::registration::TypeIndex::INVALID },
        )
    } else {
        (
            quote! { const IS_GENERIC: bool = false; },
            quote! {
                static INDEX: std::sync::LazyLock<xecs::registration::TypeIndex> =
                std::sync::LazyLock::new(|| xecs::registration::allocate_type_index());
                *INDEX
            },
        )
    };

    let is_tag = match &ast.data {
        syn::Data::Struct(data_struct) => data_struct.fields.is_empty(),
        syn::Data::Enum(data_enum) => data_enum.variants.is_empty(),
        syn::Data::Union(_) => {
            return quote! { compile_error!("Union type not supported for components."); }.into();
        }
    };

    let data_type = if is_tag {
        quote! {
            type DataType = xecs::type_traits::Tag;
            type DescType = xecs::component::TagBuilder;
        }
    } else {
        quote! {
            type DataType = xecs::type_traits::Data;
            type DescType = xecs::component::ComponentBuilder<Self>;
        }
    };

    quote! {
        unsafe impl #impl_generics xecs::type_traits::Component for #name #ty_generics
        #where_clause
        {
            #data_type
            #is_generic
        }

        unsafe impl #impl_generics xecs::registration::ComponentId for #name #ty_generics
        #where_clause
        {
            fn type_index() -> xecs::registration::TypeIndex {
                #type_index
            }
        }
    }
    .into()
}
