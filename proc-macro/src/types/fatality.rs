use indexmap::IndexMap;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    DataEnum, DataStruct, Ident, ImplGenerics, Pat, TypeGenerics, Variant,
    WhereClause,
};

use crate::types::{
    DeriveInput, ResolutionMode, abs_helper_path, enum_variant_to_pattern,
    struct_to_pattern,
};

/// Implement `trait Fatality` for `ident`.
fn enum_impl(
    ident: &Ident,
    impl_generics: &ImplGenerics,
    type_generics: &TypeGenerics,
    where_clause: Option<&WhereClause>,
    pattern_lut: &IndexMap<Variant, Pat>,
    resolution_lut: &IndexMap<Variant, ResolutionMode>,
) -> TokenStream {
    let pat = pattern_lut.values();
    let resolution = resolution_lut.values();

    let fatality_trait =
        abs_helper_path(Ident::new("Fatality", ident.span()), ident.span());
    quote! {
        #[automatically_derived]
        impl #impl_generics #fatality_trait for #ident #type_generics #where_clause {
            fn is_fatal(&self) -> bool {
                match self {
                    #( #pat => #resolution, )*
                }
            }
        }
    }
}

pub(crate) fn enum_gen(
    mut item: DeriveInput<DataEnum>,
) -> syn::Result<TokenStream> {
    let mut resolution_lut = IndexMap::new();
    let mut pattern_lut = IndexMap::new();

    // if there is not a single fatal annotation, we can just replace `#[fatality]` with `#[derive(::thiserror::Error, Debug)]`
    // without the intermediate type. But impl `trait Fatality` on-top.
    for variant in item.data.variants.iter_mut() {
        let resolution_mode =
            ResolutionMode::extract_from_variant_attrs(variant)?;

        // Obtain the patterns for each variant, and the resolution, which can either
        // be `forward`, `true`, or `false`
        // as used in the `trait Fatality`.
        let (pattern, resolution_mode) =
            enum_variant_to_pattern(variant, resolution_mode)?;
        if let ResolutionMode::Forward(_, None) = resolution_mode {
            unreachable!("Must have an ident. qed")
        }
        resolution_lut.insert(variant.clone(), resolution_mode);
        pattern_lut.insert(variant.clone(), pattern);
    }
    let (impl_generics, type_generics, where_clause) =
        item.generics.split_for_impl();

    Ok(enum_impl(
        &item.ident,
        &impl_generics,
        &type_generics,
        where_clause,
        &pattern_lut,
        &resolution_lut,
    ))
}

/// Implement `trait Fatality` for `ident`.
fn struct_impl(
    ident: &Ident,
    impl_generics: &ImplGenerics,
    type_generics: &TypeGenerics,
    where_clause: Option<&WhereClause>,
    resolution: &ResolutionMode,
) -> TokenStream {
    let fatality_trait =
        abs_helper_path(Ident::new("Fatality", ident.span()), ident.span());
    let resolution = match resolution {
        ResolutionMode::Forward(_fwd, field) => {
            let field = field
                .as_ref()
                .expect("Ident must be filled at this point. qed");
            quote! {
                #fatality_trait :: is_fatal( & self. #field )
            }
        }
        rm => quote! {
            #rm
        },
    };
    quote! {
        #[automatically_derived]
        impl #impl_generics #fatality_trait for #ident #type_generics #where_clause {
            fn is_fatal(&self) -> bool {
                #resolution
            }
        }
    }
}

pub(crate) fn struct_gen(
    mut item: DeriveInput<DataStruct>,
) -> syn::Result<proc_macro2::TokenStream> {
    let resolution_mode = ResolutionMode::extract_from_struct_attrs(&mut item)?;

    let (_pat, resolution_mode) = struct_to_pattern(&item, resolution_mode)?;
    let (impl_generics, type_generics, where_clause) =
        item.generics.split_for_impl();

    Ok(struct_impl(
        &item.ident,
        &impl_generics,
        &type_generics,
        where_clause,
        &resolution_mode,
    ))
}
