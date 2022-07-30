use proc_macro2::{Span, TokenStream};
use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;
use syn::parenthesized;
use syn::parse::{Parse, ParseStream};
use syn::parse_macro_input;
use syn::punctuated::Punctuated;
use syn::DeriveInput;
use syn::Ident;
use syn::Token;

use crate::Sharable::{Mut, Ref};
use for_ch::for_ch;

const VIEW_AS: &str = "view_as";
const REF_IN: &str = "ref_in";
const MUT_IN: &str = "mut_in";

struct IdentTuple {
    pub _paren_token: Option<syn::token::Paren>,
    pub elems: Punctuated<Ident, Token![,]>,
}

enum Sharable {
    Ref,
    Mut,
}

impl Parse for IdentTuple {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self {
                _paren_token: None,
                elems: Default::default(),
            });
        }

        let content;
        let par = parenthesized!(content in input);

        Ok(Self {
            _paren_token: Some(par),
            elems: Punctuated::<Ident, Token![,]>::parse_terminated(&content)?,
        })
    }
}

#[proc_macro_derive(Views, attributes(view_as, mut_in, ref_in))]
pub fn views_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let deriving = parse_macro_input!(input as DeriveInput);
    views_derive_impl(deriving)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn views_derive_impl(input: syn::DeriveInput) -> syn::Result<TokenStream> {
    let gens_with_bounds = Vec::from_iter(input.generics.params);
    let gens = elide_generics_bounds(&gens_with_bounds);
    let fields = guard_named_struct(input.data)?;
    let view_type_names = view_type_names_from_attrs(&input.attrs)?;

    let mut view_structs = HashMap::new();
    for view_name in view_type_names {
        let view_fields = view_type_fields(&view_name, &fields)?;
        view_structs.insert(view_name, view_fields);
    }

    let mut result = TokenStream::new();
    for (view_name, view_fields) in view_structs {
        let the_struct = construct_view_type(
            &view_name,
            &view_fields,
            &input.vis,
            &gens_with_bounds,
            &input.generics.where_clause,
        );

        let the_impl = construct_view_type_impl(
            &view_name,
            &view_fields,
            &gens_with_bounds,
            &gens,
            &input.generics.where_clause,
        );

        let the_ctor = construct_view_type_ctor(&view_name, &view_fields);

        result.extend(the_struct);
        result.extend(the_impl);
        result.extend(the_ctor);
    }

    Ok(result)
}

fn guard_named_struct(ty: syn::Data) -> syn::Result<syn::FieldsNamed> {
    use syn::{Data::*, DataStruct, Fields::*};
    if let Struct(DataStruct {
        fields: Named(fields),
        ..
    }) = ty
    {
        return Ok(fields);
    }

    Err(syn::Error::new(
        Span::call_site(),
        "`view_as` can only apply on named struct",
    ))
}

fn elide_generics_bounds(gens: &[syn::GenericParam]) -> Vec<syn::GenericParam> {
    use syn::GenericParam::*;
    use syn::LifetimeDef;
    use syn::TypeParam;
    gens.iter()
        .map(|param| match param.clone() {
            Type(ty) => Type(TypeParam {
                attrs: Default::default(),
                bounds: Default::default(),
                ..ty
            }),
            Lifetime(lifetime) => Lifetime(LifetimeDef {
                bounds: Default::default(),
                attrs: Default::default(),
                ..lifetime
            }),
            c => c,
        })
        .collect()
}

fn view_type_names_from_attrs(attrs: &[syn::Attribute]) -> syn::Result<HashSet<String>> {
    let mut names = HashSet::new();

    for_ch! {
        for attr in attrs;
        if attr.path.is_ident(&Ident::new(VIEW_AS, Span::call_site()));
        let idents = syn::parse2::<IdentTuple>(attr.tokens.to_owned())?;
        for ident in idents.elems;
        names.insert(ident.to_string());
    }

    Ok(names)
}

fn view_type_fields(
    view_name: &str,
    original_ty_fields: &syn::FieldsNamed,
) -> syn::Result<HashMap<String, (syn::Visibility, Sharable, syn::Type)>> {
    let mut res = HashMap::new();

    for_ch! {
        for field in &original_ty_fields.named;
        for attr in &field.attrs;
        for_ch! {
            if attr.path.is_ident(&Ident::new(REF_IN, Span::call_site()));
            let view_idents = syn::parse2::<IdentTuple>(attr.tokens.to_owned())?;
            for view_ident in view_idents.elems;
            if &view_ident.to_string() == view_name;
            let field_name = field.ident.as_ref().unwrap().to_string();
            res.insert(field_name,  (field.vis.clone(), Ref, field.ty.clone()));
        };

        for_ch! {
            if attr.path.is_ident(&Ident::new(MUT_IN, Span::call_site()));
            let view_idents = syn::parse2::<IdentTuple>(attr.tokens.to_owned())?;
            for view_ident in view_idents.elems;
            if &view_ident.to_string() == view_name;
            let field_name = field.ident.as_ref().unwrap().to_string();
            res.insert(field_name,  (field.vis.clone(), Mut, field.ty.clone()));
        };
    }

    Ok(res)
}

fn construct_view_type(
    view_name: &str,
    fields: &HashMap<String, (syn::Visibility, Sharable, syn::Type)>,
    vis: &syn::Visibility,
    gens: &[syn::GenericParam],
    where_clause: &Option<syn::WhereClause>,
) -> TokenStream {
    let view_name = syn::Ident::new(view_name, Span::call_site());
    let ref_lifetime = syn::Lifetime::new("'__ref__", Span::call_site());
    let mut_lifetime = syn::Lifetime::new("'__mut__", Span::call_site());

    let fields = fields
        .iter()
        .map(|(field_name, (vis, share, ty))| {
            let field_name = syn::Ident::new(field_name, Span::call_site());
            match share {
                Ref => quote::quote! {
                    #vis #field_name: &#ref_lifetime #ty
                },
                Mut => quote::quote! {
                    #vis #field_name: &#mut_lifetime mut #ty
                },
            }
        })
        .collect::<Vec<_>>();

    quote::quote! {
        #[allow(snake_case)]
        #vis struct #view_name <#ref_lifetime, #mut_lifetime, #(#gens,)*>
        #where_clause
        {
            #(#fields,)*

            #[doc(hidden)]
            _marker: ::core::marker::PhantomData<(&#ref_lifetime (), &#mut_lifetime mut ())>,
        }
    }
}

fn construct_view_type_impl(
    view_name: &str,
    fields: &HashMap<String, (syn::Visibility, Sharable, syn::Type)>,
    gens: &[syn::GenericParam],
    gens_without_bounds: &[syn::GenericParam],
    where_clause: &Option<syn::WhereClause>,
) -> TokenStream {
    let view_name = syn::Ident::new(view_name, Span::call_site());
    let ref_lifetime = syn::Lifetime::new("'__ref__", Span::call_site());
    let mut_lifetime = syn::Lifetime::new("'__mut__", Span::call_site());

    let fields = fields
        .iter()
        .map(|(field_name, (_, share, _))| {
            let field_name = syn::Ident::new(field_name, Span::call_site());
            match share {
                Ref => quote::quote! {
                    #field_name: & self . #field_name
                },
                Mut => quote::quote! {
                    #field_name: &mut self . #field_name
                },
            }
        })
        .collect::<Vec<_>>();

    quote::quote! {
        impl < #ref_lifetime, #mut_lifetime, #(#gens,)* >
        #view_name < #ref_lifetime, #mut_lifetime, #(#gens_without_bounds,)* >
        #where_clause
        {
            pub fn reborrow<'__brw__>(&'__brw__ mut self) -> #view_name < #ref_lifetime, '__brw__, #(#gens_without_bounds,)* > {
                #view_name {
                    #(#fields,)*
                    _marker : ::core::marker::PhantomData,
                }
            }
        }
    }
}

fn construct_view_type_ctor(
    view_name: &str,
    fields: &HashMap<String, (syn::Visibility, Sharable, syn::Type)>,
) -> TokenStream {
    let view_name = syn::Ident::new(view_name, Span::call_site());
    let ctor_name = syn::Ident::new(&format!("{view_name}_ctor"), Span::call_site());
    let fields = fields
        .iter()
        .map(|(field_name, (_, share, _))| {
            let field_name = syn::Ident::new(field_name, Span::call_site());
            match share {
                Ref => quote::quote! {
                    #field_name: & $e . #field_name
                },
                Mut => quote::quote! {
                    #field_name: &mut $e . #field_name
                },
            }
        })
        .collect::<Vec<_>>();

    quote::quote! {
        #[macro_export]
        macro_rules! #ctor_name {
            ($e: expr) => {
                #view_name {
                    #(#fields,)*
                    _marker : ::core::marker::PhantomData,
                }
            };
        }
    }
}
