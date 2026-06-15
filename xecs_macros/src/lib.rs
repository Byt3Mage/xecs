use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{
    DeriveInput, Fields, GenericArgument, GenericParam, Ident, Lifetime, LitInt, PathArguments, Result, Token, Type,
    TypeReference,
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
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

#[proc_macro]
pub fn components(input: TokenStream) -> TokenStream {
    let components = parse_macro_input!(input as Components);
    quote! { #components }.into()
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

#[proc_macro_derive(Row, attributes(component))]
pub fn row(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let syn::Data::Struct(s) = &input.data else {
        return quote! { compile_error!("xecs: only structs are supported for rows"); }.into();
    };
    let Fields::Named(fields) = &s.fields else {
        return quote! { compile_error!("xecs: only structs with named fields are supported for rows"); }.into();
    };

    let struct_name = &input.ident;

    let field_name = fields
        .named
        .iter()
        .map(|f| f.ident.clone().unwrap())
        .collect::<Vec<_>>();
    let field_ty = fields.named.iter().map(|f| f.ty.clone()).collect::<Vec<_>>();
    let field_idx = (0..field_name.len()).collect::<Vec<_>>();

    // --- generics ----------------------------------------------------------

    let num_struct_lifetimes = input.generics.lifetimes().count();

    let type_const_params: Vec<GenericParam> = input
        .generics
        .params
        .iter()
        .filter(|p| !matches!(p, GenericParam::Lifetime(_)))
        .cloned()
        .collect();

    let type_const_args: Vec<proc_macro2::TokenStream> = input
        .generics
        .params
        .iter()
        .filter_map(|p| match p {
            GenericParam::Type(t) => {
                let id = &t.ident;
                Some(quote!(#id))
            }
            GenericParam::Const(c) => {
                let id = &c.ident;
                Some(quote!(#id))
            }
            GenericParam::Lifetime(_) => None,
        })
        .collect();

    let user_where = input.generics.where_clause.clone();

    // Build `StructName<lt, lt, ..., T, N>` with one `lt` per lifetime slot.
    let struct_path = |lt: &Lifetime| -> proc_macro2::TokenStream {
        let lts = (0..num_struct_lifetimes).map(|_| lt);
        if num_struct_lifetimes == 0 && type_const_args.is_empty() {
            quote!(#struct_name)
        } else {
            quote!(#struct_name< #(#lts,)* #(#type_const_args),* >)
        }
    };

    // Normalize every lifetime in each field type to `lt`.
    let fields_with_lt = |lt: &Lifetime| -> Vec<Type> {
        field_ty
            .iter()
            .cloned()
            .map(|mut ty| {
                rewrite_lifetimes(&mut ty, lt);
                ty
            })
            .collect()
    };

    // Lifetimes, each scoped to where it is bound:
    let impl_lt: Lifetime = parse_quote!('__r); // impl binder / Self type
    let gat_lt: Lifetime = parse_quote!('c); // Get / get
    let col_lt: Lifetime = parse_quote!('t); // Columns / borrow_columns

    let tc = &type_const_params;
    let impl_generics = quote!(<#impl_lt #(, #tc)* >);

    let self_ty = struct_path(&impl_lt); // StructName<'__r, ...>
    let get_ty = struct_path(&gat_lt); // StructName<'c, ...>

    // Field-type normalizations per scope.
    let columns_field_ty = fields_with_lt(&col_lt); // 't  → Columns / borrow_columns
    let row_field_ty = fields_with_lt(&gat_lt); // 'c  → get body
    let access_field_ty = fields_with_lt(&impl_lt); // '__r → TRow access + where-clause

    // Row impl: just the user's where-clause (if any).
    let row_where = user_where.as_ref().map(|w| quote!(#w));

    quote! {
        impl #impl_generics xecs::query::iter::Row for #self_ty
        #row_where
        {
            type Get<'c> = #get_ty;

            type Columns<'t> = ( #(
                <#columns_field_ty as xecs::query::iter::Field>::Column<'t>,
            )* );

            const ACCESSES: &'static [xecs::access::AccessType] = &[#(
                <#access_field_ty as xecs::component::ComponentAccess>::ACCESS,
            )*];

            fn columns<'t>(
                iter: &'t xecs::query::iter::TableIter<'t>,
            ) -> Self::Columns<'t> {
                ( #(
                    <#columns_field_ty as xecs::query::iter::Field>::column(iter, #field_idx),
                )* )
            }

            unsafe fn get<'c>(
                column: &mut Self::Columns<'c>,
                row: usize,
            ) -> Self::Get<'c> {
                let ( #(#field_name,)* ) = column;
                unsafe {
                    #(
                        let #field_name =
                            <#row_field_ty as xecs::query::iter::Field>::row(#field_name, row);
                    )*
                    #struct_name { #(#field_name),* }
                }
            }
        }
    }
    .into()
}

/// Recursively rewrite every lifetime inside `ty` to `target`.
fn rewrite_lifetimes(ty: &mut Type, target: &Lifetime) {
    match ty {
        Type::Reference(TypeReference { lifetime, elem, .. }) => {
            *lifetime = Some(target.clone());
            rewrite_lifetimes(elem, target);
        }
        Type::Path(tp) => {
            if let Some(qself) = &mut tp.qself {
                rewrite_lifetimes(&mut qself.ty, target);
            }
            for seg in &mut tp.path.segments {
                if let PathArguments::AngleBracketed(args) = &mut seg.arguments {
                    for arg in &mut args.args {
                        match arg {
                            GenericArgument::Lifetime(lt) => *lt = target.clone(),
                            GenericArgument::Type(inner) => rewrite_lifetimes(inner, target),
                            _ => {}
                        }
                    }
                }
            }
        }
        Type::Tuple(t) => {
            for elem in &mut t.elems {
                rewrite_lifetimes(elem, target);
            }
        }
        Type::Slice(s) => rewrite_lifetimes(&mut s.elem, target),
        Type::Array(a) => rewrite_lifetimes(&mut a.elem, target),
        Type::Group(g) => rewrite_lifetimes(&mut g.elem, target),
        Type::Paren(p) => rewrite_lifetimes(&mut p.elem, target),
        Type::Ptr(p) => rewrite_lifetimes(&mut p.elem, target),
        _ => {}
    }
}
