use indexmap::IndexMap;
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::{
    DataEnum, DataStruct, Fields, Ident, ItemEnum, ItemStruct, Pat, PatIdent,
    Token, Variant, punctuated::Punctuated, spanned::Spanned as _,
};

use crate::{
    split::{SplitVariant, opts::Opts},
    types::{
        DeriveInput, ResolutionMode, abs_helper_path, enum_variant_to_pattern,
        struct_to_pattern,
        unnamed_fields_variant_pattern_constructor_binding_name,
    },
};

#[derive(Hash, Debug)]
struct VariantPattern(Variant);

impl ToTokens for VariantPattern {
    fn to_tokens(&self, ts: &mut TokenStream) {
        let variant_name = &self.0.ident;
        let variant_fields = &self.0.fields;

        match variant_fields {
            Fields::Unit => {
                ts.extend(quote! { #variant_name });
            }
            Fields::Unnamed(unnamed) => {
                let pattern = unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(ith, _field)| {
                        Pat::Ident(PatIdent {
                            attrs: vec![],
                            by_ref: None,
                            mutability: None,
                            ident: unnamed_fields_variant_pattern_constructor_binding_name(ith),
                            subpat: None,
                        })
                    })
                    .collect::<Punctuated<Pat, Token![,]>>();
                ts.extend(quote! { #variant_name(#pattern) });
            }
            Fields::Named(named) => {
                let pattern = named
                    .named
                    .iter()
                    .map(|field| {
                        Pat::Ident(PatIdent {
                            attrs: vec![],
                            by_ref: None,
                            mutability: None,
                            ident: field
                                .ident
                                .clone()
                                .expect("Named field has a name. qed"),
                            subpat: None,
                        })
                    })
                    .collect::<Punctuated<Pat, Token![,]>>();
                ts.extend(quote! { #variant_name{ #pattern } });
            }
        };
    }
}

/// Constructs an enum variant.
#[derive(Hash, Debug)]
struct VariantConstructor(Variant);

impl ToTokens for VariantConstructor {
    fn to_tokens(&self, ts: &mut TokenStream) {
        let variant_name = &self.0.ident;
        let variant_fields = &self.0.fields;
        ts.extend(match variant_fields {
            Fields::Unit => quote! { #variant_name },
            Fields::Unnamed(unnamed) => {
                let constructor = unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(ith, _field)| {
                        unnamed_fields_variant_pattern_constructor_binding_name(
                            ith,
                        )
                    })
                    .collect::<Punctuated<Ident, Token![,]>>();
                quote! { #variant_name (#constructor) }
            }
            Fields::Named(named) => {
                let constructor = named
                    .named
                    .iter()
                    .map(|field| {
                        field
                            .ident
                            .clone()
                            .expect("Named must have named fields. qed")
                    })
                    .collect::<Punctuated<Ident, Token![,]>>();
                quote! { #variant_name { #constructor } }
            }
        });
    }
}

/// Generate the Jfyi and Fatal sub enums.
///
/// `fatal_variants` and `jfyi_variants` cover _all_ variants, if they are forward, they are part of both slices.
/// `forward_variants` enlists all variants that
fn enum_impl(
    split_opts: Opts,
    original: DeriveInput<DataEnum>,
    resolution_lut: &IndexMap<Variant, ResolutionMode>,
    jfyi_variants: &[Variant],
    fatal_variants: &[Variant],
) -> Result<TokenStream, syn::Error> {
    let span = original.data.brace_token.span.join();

    let split_trait = abs_helper_path(Ident::new("Split", span), span);

    let original_ident = &original.ident;

    // Generate the splitable types:
    //   Fatal
    let fatal_ident = split_opts
        .ident(SplitVariant::Fatal)
        .unwrap_or_else(|| format!("Fatal{}", original.ident));
    let fatal_ident = Ident::new(fatal_ident.as_str(), span);
    let fatal = {
        let attrs = split_opts.split_error_attrs(
            Span::call_site(),
            &original.attrs,
            SplitVariant::Fatal,
        );
        ItemEnum {
            attrs,
            vis: original.vis.clone(),
            enum_token: original.data.enum_token,
            ident: fatal_ident.clone(),
            generics: original.generics.clone(),
            brace_token: original.data.brace_token,
            variants: fatal_variants.iter().cloned().collect(),
        }
    };

    //  Informational (just for your information)
    let jfyi_ident = split_opts
        .ident(SplitVariant::Jfyi)
        .unwrap_or_else(|| format!("Jfyi{}", original.ident));
    let jfyi_ident = Ident::new(jfyi_ident.as_str(), span);
    let jfyi = {
        let attrs = split_opts.split_error_attrs(
            Span::call_site(),
            &original.attrs,
            SplitVariant::Jfyi,
        );
        ItemEnum {
            attrs,
            vis: original.vis,
            enum_token: original.data.enum_token,
            ident: jfyi_ident.clone(),
            generics: original.generics.clone(),
            brace_token: original.data.brace_token,
            variants: jfyi_variants.iter().cloned().collect(),
        }
    };

    let fatal_patterns = fatal_variants
        .iter()
        .map(|variant| VariantPattern(variant.clone()))
        .collect::<Vec<_>>();
    let jfyi_patterns = jfyi_variants
        .iter()
        .map(|variant| VariantPattern(variant.clone()))
        .collect::<Vec<_>>();

    let fatal_constructors = fatal_variants
        .iter()
        .map(|variant| VariantConstructor(variant.clone()))
        .collect::<Vec<_>>();
    let jfyi_constructors = jfyi_variants
        .iter()
        .map(|variant| VariantConstructor(variant.clone()))
        .collect::<Vec<_>>();

    let mut ts = TokenStream::new();

    ts.extend(quote! {
        #fatal

        #[automatically_derived]
        impl ::std::convert::From< #fatal_ident> for #original_ident {
            fn from(fatal: #fatal_ident) -> Self {
                match fatal {
                    // Fatal
                    #( #fatal_ident :: #fatal_patterns => Self:: #fatal_constructors, )*
                }
            }
        }

        #jfyi

        #[automatically_derived]
        impl ::std::convert::From< #jfyi_ident> for #original_ident {
            fn from(jfyi: #jfyi_ident) -> Self {
                match jfyi {
                    // JFYI
                    #( #jfyi_ident :: #jfyi_patterns => Self:: #jfyi_constructors, )*
                }
            }
        }
    });

    // Handle `forward` annotations.
    let trait_fatality =
        abs_helper_path(format_ident!("Fatality"), Span::call_site());

    // add a a `fatal` variant
    let fatal_patterns_w_if_maybe = fatal_variants
        .iter()
        .map(|variant| {
            let pat = VariantPattern(variant.clone());
            if let Some(ResolutionMode::Forward(_fwd_kw, member)) = resolution_lut.get(variant) {
                let ident = match member
                    .clone()
                    .expect("Forward mode must have an ident at this point. qed")
                {
                    syn::Member::Named(ident) => ident,
                    syn::Member::Unnamed(idx) => {
                        unnamed_fields_variant_pattern_constructor_binding_name(idx.index as usize)
                    }
                };
                quote! { #pat if < _ as #trait_fatality >::is_fatal( & #ident ) }
            } else {
                pat.into_token_stream()
            }
        })
        .collect::<Vec<_>>();

    let jfyi_patterns_w_if_maybe = jfyi_variants
        .iter()
        .map(|variant| {
            let pat = VariantPattern(variant.clone());
            assert!(
                resolution_lut.get(variant).is_some(),
                "cannot be annotated as fatal when in the JFYI slice. qed"
            );
            pat.into_token_stream()
        })
        .collect::<Vec<_>>();

    let split_trait_impl = quote! {
        #[automatically_derived]
        impl #split_trait for #original_ident {
            type Fatal = #fatal_ident;
            type Jfyi = #jfyi_ident;

            fn split(self) -> ::std::result::Result<Self::Jfyi, Self::Fatal> {
                match self {
                    // Fatal
                    #( Self :: #fatal_patterns_w_if_maybe => Err(#fatal_ident :: #fatal_constructors), )*
                    // JFYI
                    #( Self :: #jfyi_patterns_w_if_maybe => Ok(#jfyi_ident :: #jfyi_constructors), )*
                    // issue: https://github.com/rust-lang/rust/issues/93611#issuecomment-1028844586
                    // #( Self :: #forward_patterns => unreachable!("`Fatality::is_fatal` can only be `true` or `false`, which are covered. qed"), )*
                }
            }
        }
    };
    ts.extend(split_trait_impl);

    Ok(ts)
}

pub(crate) fn enum_gen(
    mut item: DeriveInput<DataEnum>,
) -> syn::Result<TokenStream> {
    let opts = Opts::from_attrs(&item.attrs)?;
    let mut resolution_lut = IndexMap::new();

    let mut jfyi_variants = Vec::new();
    let mut fatal_variants = Vec::new();

    // if there is not a single fatal annotation, we can just replace `#[fatality]` with `#[derive(::thiserror::Error, Debug)]`
    // without the intermediate type. But impl `trait Fatality` on-top.
    for variant in item.data.variants.iter_mut() {
        let resolution_mode =
            ResolutionMode::extract_from_variant_attrs(variant)?;

        // Obtain the patterns for each variant, and the resolution, which can either
        // be `forward`, `true`, or `false`
        // as used in the `trait Fatality`.
        let (_pattern, resolution_mode) =
            enum_variant_to_pattern(variant, resolution_mode)?;
        match resolution_mode {
            ResolutionMode::Forward(_, None) => {
                unreachable!("Must have an ident. qed")
            }
            ResolutionMode::Forward(_, ref _ident) => {
                jfyi_variants.push(variant.clone());
                fatal_variants.push(variant.clone());
            }
            ResolutionMode::WithExplicitBool(ref b) => {
                if b.value {
                    fatal_variants.push(variant.clone())
                } else {
                    jfyi_variants.push(variant.clone())
                }
            }
        }
        resolution_lut.insert(variant.clone(), resolution_mode);
    }

    enum_impl(opts, item, &resolution_lut, &jfyi_variants, &fatal_variants)
}

/// Mutably borrow a field by index
fn get_field_mut(
    fields: &mut syn::Fields,
    idx: usize,
) -> Option<&mut syn::Field> {
    match fields {
        syn::Fields::Named(fields) => fields.named.get_mut(idx),
        syn::Fields::Unnamed(fields) => fields.unnamed.get_mut(idx),
        syn::Fields::Unit => None,
    }
}

/// Generate the Jfyi and Fatal sub structs.
///
/// `fatal_variants` and `jfyi_variants` cover _all_ variants, if they are forward, they are part of both slices.
/// `forward_variants` enlists all variants that
fn struct_impl(
    split_opts: Opts,
    original: &DeriveInput<DataStruct>,
    split_field_idx: usize,
) -> Result<TokenStream, syn::Error> {
    let span = original.data.fields.span();

    let split_trait = abs_helper_path(Ident::new("Split", span), span);

    let original_ident = original.ident.clone();

    let split_field = original.data.fields.iter().nth(split_field_idx).unwrap();
    let split_field_projector: syn::Member = match split_field.ident.clone() {
        Some(ident) => ident.into(),
        None => split_field_idx.into(),
    };
    let split_field_ty = &split_field.ty;

    // Generate the splitable types:
    //   Fatal
    let fatal_ident = split_opts
        .ident(SplitVariant::Fatal)
        .unwrap_or_else(|| format!("Fatal{}", original.ident));
    let fatal_ident = Ident::new(fatal_ident.as_str(), span);
    let fatal = {
        let attrs = split_opts.split_error_attrs(
            Span::call_site(),
            &original.attrs,
            SplitVariant::Fatal,
        );
        let mut fields = original.data.fields.clone();
        if let Some(field) = get_field_mut(&mut fields, split_field_idx) {
            let mut split_fatal_path = split_trait.clone();
            split_fatal_path.segments.push(syn::PathSegment::from(
                syn::Ident::new("Fatal", Span::call_site()),
            ));
            field.ty = syn::Type::Path(syn::TypePath {
                qself: Some(syn::QSelf {
                    lt_token: syn::token::Lt(Span::call_site()),
                    ty: Box::new(split_field.ty.clone()),
                    position: split_fatal_path.segments.len() - 1,
                    as_token: Some(syn::token::As(Span::call_site())),
                    gt_token: syn::token::Gt(Span::call_site()),
                }),
                path: split_fatal_path,
            });
        }
        ItemStruct {
            attrs,
            vis: original.vis.clone(),
            struct_token: original.data.struct_token,
            ident: fatal_ident.clone(),
            generics: original.generics.clone(),
            fields,
            semi_token: original.data.semi_token,
        }
    };
    //  Informational (just for your information)
    let jfyi_ident = split_opts
        .ident(SplitVariant::Jfyi)
        .unwrap_or_else(|| format!("Jfyi{}", original.ident));
    let jfyi_ident = Ident::new(jfyi_ident.as_str(), span);
    let jfyi = {
        let attrs = split_opts.split_error_attrs(
            Span::call_site(),
            &original.attrs,
            SplitVariant::Jfyi,
        );
        let mut fields = original.data.fields.clone();
        if let Some(field) = get_field_mut(&mut fields, split_field_idx) {
            let mut split_jfyi_path = split_trait.clone();
            split_jfyi_path.segments.push(syn::PathSegment::from(
                syn::Ident::new("Jfyi", Span::call_site()),
            ));
            field.ty = syn::Type::Path(syn::TypePath {
                qself: Some(syn::QSelf {
                    lt_token: syn::token::Lt(Span::call_site()),
                    ty: Box::new(split_field.ty.clone()),
                    position: split_jfyi_path.segments.len() - 1,
                    as_token: Some(syn::token::As(Span::call_site())),
                    gt_token: syn::token::Gt(Span::call_site()),
                }),
                path: split_jfyi_path,
            });
        }
        ItemStruct {
            attrs,
            vis: original.vis.clone(),
            struct_token: original.data.struct_token,
            ident: jfyi_ident.clone(),
            generics: original.generics.clone(),
            fields,
            semi_token: original.data.semi_token,
        }
    };

    let mut ts = TokenStream::new();

    let non_split_field_projectors: Vec<_> = original
        .data
        .fields
        .iter()
        .enumerate()
        .filter_map(|(field_idx, field)| {
            if field_idx != split_field_idx {
                let field_projector: syn::Member = match field.ident.clone() {
                    Some(ident) => ident.into(),
                    None => field_idx.into(),
                };
                Some(field_projector)
            } else {
                None
            }
        })
        .collect();

    ts.extend(quote! {
        #fatal

        #[automatically_derived]
        impl ::std::convert::From< #fatal_ident> for #original_ident {
            fn from(fatal: #fatal_ident) -> Self {
                Self {
                    #(#non_split_field_projectors: fatal.#non_split_field_projectors,)*
                    #split_field_projector: #split_field_ty::from(fatal.#split_field_projector),
                }
            }
        }

        #jfyi

        #[automatically_derived]
        impl ::std::convert::From< #jfyi_ident> for #original_ident {
            fn from(jfyi: #jfyi_ident) -> Self {
                Self {
                    #(#non_split_field_projectors: jfyi.#non_split_field_projectors,)*
                    #split_field_projector: #split_field_ty::from(jfyi.#split_field_projector),
                }
            }
        }
    });

    let split_trait_impl = quote! {
        #[automatically_derived]
        impl #split_trait for #original_ident {
            type Fatal = #fatal_ident;
            type Jfyi = #jfyi_ident;

            fn split(self) -> ::std::result::Result<Self::Jfyi, Self::Fatal> {
                match #split_trait::split(self.#split_field_projector) {
                    Err(fatal) => Err(#fatal_ident {
                        #(#non_split_field_projectors: self.#non_split_field_projectors,)*
                        #split_field_projector: fatal,
                    }),
                    Ok(jfyi) => Ok(#jfyi_ident {
                        #(#non_split_field_projectors: self.#non_split_field_projectors,)*
                        #split_field_projector: jfyi,
                    }),
                }
            }
        }
    };
    ts.extend(split_trait_impl);

    Ok(ts)
}

pub(crate) fn struct_gen(
    span: Span,
    mut item: DeriveInput<DataStruct>,
) -> syn::Result<proc_macro2::TokenStream> {
    let split_opts = Opts::from_attrs(&item.attrs)?;
    let resolution_mode = ResolutionMode::extract_from_struct_attrs(&mut item)?;

    let (_pat, resolution_mode) = struct_to_pattern(&item, resolution_mode)?;

    match resolution_mode {
        ResolutionMode::WithExplicitBool(_) => {
            let err_msg = "cannot specify a fatality for splitable structs";
            return Err(syn::Error::new(span, err_msg));
        }
        ResolutionMode::Forward(_, _) => (),
    }
    if item.data.fields.is_empty() {
        let err_msg = "cannot derive `Split` for a unit struct";
        return Err(syn::Error::new(span, err_msg));
    }
    let Some(source_field_idx) = item
        .data
        .fields
        .iter()
        .position(|field| {
            field.attrs.iter().any(|field_attr| {
                matches!(field_attr.style, syn::AttrStyle::Outer)
                    && field_attr.meta.require_path_only().is_ok_and(
                        |field_attr_path| {
                            field_attr_path.is_ident("from")
                                || field_attr_path.is_ident("source")
                        },
                    )
            })
        })
        .or_else(|| {
            item.data.fields.iter().position(|field| {
                field
                    .ident
                    .as_ref()
                    .is_some_and(|field_ident| field_ident == "source")
            })
        })
        .or_else(|| match &item.data.fields {
            syn::Fields::Unnamed(fields) if !fields.unnamed.is_empty() => {
                Some(0)
            }
            _ => None,
        })
    else {
        return Err(syn::Error::new(
            span,
            "cannot use `splitable` on a `struct` without a source field",
        ));
    };
    struct_impl(split_opts, &item, source_field_idx)
}
