use quote::{ToTokens, format_ident, quote};
use syn::{
    Ident, LitInt, Result, Token, Type,
    parse::{self, Parse, ParseStream},
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
pub fn all_tuples(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
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
        if self.items.len() > 1 {
            let items = &self.items;
            tokens.extend(quote! {(#items)});
        } else {
            let item = &self.items[0];
            tokens.extend(quote! { #item });
        }
    }
}

#[proc_macro]
pub fn params(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let params = parse_macro_input!(input as Params);
    quote! { #params }.into()
}
