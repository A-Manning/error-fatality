use indexmap::IndexMap;
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::{
    DataEnum, DataStruct, Fields, Ident, ItemEnum, ItemStruct, Pat, PatIdent, Token, Variant,
    punctuated::Punctuated, spanned::Spanned as _,
};

use crate::types::{
    DeriveInput, ResolutionMode, abs_helper_path, enum_variant_to_pattern, struct_to_pattern,
    unnamed_fields_variant_pattern_constructor_binding_name,
};

mod opts {
    //! Options provided via the `#[split(_)]` attribute

    use proc_macro2::Span;
    use quote::ToTokens as _;
    use syn::{
        Attribute, Meta, PathArguments, Token, punctuated::Punctuated, spanned::Spanned,
        token::PathSep,
    };

    /// Construct path segments (without arguments) from idents
    fn path_segments<'a, I>(
        span: Span,
        idents: I,
    ) -> syn::punctuated::Punctuated<syn::PathSegment, PathSep>
    where
        I: IntoIterator<Item = &'a str>,
    {
        idents
            .into_iter()
            .map(|ident| syn::PathSegment {
                ident: syn::Ident::new(ident, span),
                arguments: PathArguments::None,
            })
            .collect()
    }

    /// Construct path from root, from idents.
    /// ie. `path_from_root(["core", "fmt", "Debug"])` will construct the path
    /// `::core::fmt::Debug`.
    fn path_from_root<'a, I>(span: Span, idents: I) -> syn::Path
    where
        I: IntoIterator<Item = &'a str>,
    {
        syn::Path {
            leading_colon: Some(syn::token::PathSep::default()),
            segments: path_segments(span, idents),
        }
    }

    /// ::std::fmt::Debug
    fn debug_path_from_root(span: Span) -> syn::Path {
        path_from_root(span, ["std", "fmt", "Debug"])
    }

    /// ::thiserror::Error
    fn thiserror_path_from_root(span: Span) -> syn::Path {
        path_from_root(span, ["thiserror", "Error"])
    }

    /// Default derives for split errors
    fn default_split_derives(span: Span) -> [syn::Path; 2] {
        [debug_path_from_root(span), thiserror_path_from_root(span)]
    }

    /// Construct an outer attribute from attribute content
    fn outer_attr(meta: syn::Meta) -> syn::Attribute {
        syn::Attribute {
            pound_token: Default::default(),
            style: syn::AttrStyle::Outer,
            bracket_token: Default::default(),
            meta,
        }
    }

    /// Construct a derive attribute, with the specified derives
    fn derive_attr(span: Span, derives: Punctuated<syn::Meta, syn::Token![,]>) -> syn::Attribute {
        let meta_path = syn::PathSegment {
            ident: syn::Ident::new("derive", span),
            arguments: syn::PathArguments::None,
        };
        let meta = syn::Meta::List(syn::MetaList {
            path: meta_path.into(),
            delimiter: syn::MacroDelimiter::Paren(syn::token::Paren::default()),
            tokens: derives.to_token_stream(),
        });
        outer_attr(meta)
    }

    /// Default derive attr for split errors
    fn default_split_derive_attr(span: Span) -> syn::Attribute {
        let derives = default_split_derives(span)
            .into_iter()
            .map(syn::Meta::Path)
            .collect();
        derive_attr(span, derives)
    }

    #[derive(Clone, Debug)]
    #[repr(transparent)]
    pub(in crate::types::split) struct Attrs(Option<Punctuated<Meta, Token![,]>>);

    impl Attrs {
        /// Generate attrs for split errors
        pub fn split_error_attrs(self, span: Span, original_attrs: &[Attribute]) -> Vec<Attribute> {
            if let Some(attrs) = self.0 {
                attrs.into_iter().map(outer_attr).collect()
            } else {
                let derive_attr = default_split_derive_attr(span);
                let retained_attrs = original_attrs
                    .iter()
                    .filter(|attr| !attr.path().is_ident("split"))
                    .cloned();
                std::iter::once(derive_attr).chain(retained_attrs).collect()
            }
        }
    }

    /// Options provided via the `#[split(_)]` attribute
    #[derive(Clone, Debug)]
    #[repr(transparent)]
    pub(in crate::types::split) struct Opts {
        pub attrs: Attrs,
    }

    impl Opts {
        const INVALID_ATTR_PATH_ERR_MSG: &str = "invalid attribute path";

        const INVALID_SYNTAX_ERR_MSG: &str = "invalid syntax for `split` attribute";

        const MULTIPLE_ATTRS_ERR_MSG: &str = "cannot set attrs multiple times";

        /// Parse from a single attribute. Returns an error if the attribute path
        /// does not match.
        fn from_attr(attr: &syn::Attribute) -> syn::Result<Self> {
            let mut res = Self { attrs: Attrs(None) };
            if !attr.path().is_ident("split") {
                return Err(syn::Error::new(
                    attr.span(),
                    Self::INVALID_ATTR_PATH_ERR_MSG,
                ));
            };
            let nested = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
            if nested.is_empty() {
                return Err(syn::Error::new(nested.span(), Self::INVALID_SYNTAX_ERR_MSG));
            }
            for meta in nested {
                match meta {
                    Meta::List(meta) if meta.path.is_ident("attrs") => {
                        if res.attrs.0.is_none() {
                            res.attrs.0 = Some(meta.parse_args_with(
                                Punctuated::<Meta, Token![,]>::parse_terminated,
                            )?);
                        } else {
                            return Err(syn::Error::new(meta.span(), Self::MULTIPLE_ATTRS_ERR_MSG));
                        }
                    }
                    Meta::List(_) | Meta::NameValue(_) | Meta::Path(_) => {
                        return Err(syn::Error::new(meta.span(), Self::INVALID_SYNTAX_ERR_MSG));
                    }
                }
            }
            Ok(res)
        }

        fn extend(self, other: Self) -> syn::Result<Self> {
            let Self {
                attrs: Attrs(l_attrs),
            } = self;
            let Self {
                attrs: Attrs(r_attrs),
            } = other;
            let attrs = match (l_attrs, r_attrs) {
                (l_attrs, None) => l_attrs,
                (None, Some(r_attrs)) => Some(r_attrs),
                (Some(_), Some(r_attrs)) => {
                    return Err(syn::Error::new(
                        r_attrs.span(),
                        Self::MULTIPLE_ATTRS_ERR_MSG,
                    ));
                }
            };
            Ok(Self {
                attrs: Attrs(attrs),
            })
        }

        pub(crate) fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
            let mut res = Self { attrs: Attrs(None) };
            for attr in attrs {
                if !attr.path().is_ident("split") {
                    continue;
                }
                res = res.extend(Self::from_attr(attr)?)?;
            }
            Ok(res)
        }
    }
}
use opts::Opts;

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
                            ident: field.ident.clone().expect("Named field has a name. qed"),
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
                        unnamed_fields_variant_pattern_constructor_binding_name(ith)
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
    let fatal_ident = Ident::new(format!("Fatal{}", original.ident).as_str(), span);
    let fatal = {
        let attrs = split_opts
            .attrs
            .clone()
            .split_error_attrs(Span::call_site(), &original.attrs);
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
    let jfyi_ident = Ident::new(format!("Jfyi{}", original.ident).as_str(), span);
    let jfyi = {
        let attrs = split_opts
            .attrs
            .split_error_attrs(Span::call_site(), &original.attrs);
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

        impl ::std::convert::From< #fatal_ident> for #original_ident {
            fn from(fatal: #fatal_ident) -> Self {
                match fatal {
                    // Fatal
                    #( #fatal_ident :: #fatal_patterns => Self:: #fatal_constructors, )*
                }
            }
        }

        #jfyi

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
    let trait_fatality = abs_helper_path(format_ident!("Fatality"), Span::call_site());

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

pub(crate) fn enum_gen(mut item: DeriveInput<DataEnum>) -> syn::Result<TokenStream> {
    let opts = Opts::from_attrs(&item.attrs)?;
    let mut resolution_lut = IndexMap::new();

    let mut jfyi_variants = Vec::new();
    let mut fatal_variants = Vec::new();

    // if there is not a single fatal annotation, we can just replace `#[fatality]` with `#[derive(::thiserror::Error, Debug)]`
    // without the intermediate type. But impl `trait Fatality` on-top.
    for variant in item.data.variants.iter_mut() {
        let resolution_mode = ResolutionMode::extract_from_variant_attrs(variant)?;

        // Obtain the patterns for each variant, and the resolution, which can either
        // be `forward`, `true`, or `false`
        // as used in the `trait Fatality`.
        let (_pattern, resolution_mode) = enum_variant_to_pattern(variant, resolution_mode)?;
        match resolution_mode {
            ResolutionMode::Forward(_, None) => unreachable!("Must have an ident. qed"),
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
fn get_field_mut(fields: &mut syn::Fields, idx: usize) -> Option<&mut syn::Field> {
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
    let fatal_ident = Ident::new(format!("Fatal{}", original_ident).as_str(), span);
    let fatal = {
        let attrs = split_opts
            .attrs
            .clone()
            .split_error_attrs(Span::call_site(), &original.attrs);
        let mut fields = original.data.fields.clone();
        if let Some(field) = get_field_mut(&mut fields, split_field_idx) {
            let mut split_fatal_path = split_trait.clone();
            split_fatal_path
                .segments
                .push(syn::PathSegment::from(syn::Ident::new(
                    "Fatal",
                    Span::call_site(),
                )));
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
    let jfyi_ident = Ident::new(format!("Jfyi{}", original_ident).as_str(), span);
    let jfyi = {
        let attrs = split_opts
            .attrs
            .split_error_attrs(Span::call_site(), &original.attrs);
        let mut fields = original.data.fields.clone();
        if let Some(field) = get_field_mut(&mut fields, split_field_idx) {
            let mut split_jfyi_path = split_trait.clone();
            split_jfyi_path
                .segments
                .push(syn::PathSegment::from(syn::Ident::new(
                    "Jfyi",
                    Span::call_site(),
                )));
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

        impl ::std::convert::From< #fatal_ident> for #original_ident {
            fn from(fatal: #fatal_ident) -> Self {
                Self {
                    #(#non_split_field_projectors: fatal.#non_split_field_projectors,)*
                    #split_field_projector: #split_field_ty::from(fatal.#split_field_projector),
                }
            }
        }

        #jfyi

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
                    && field_attr
                        .meta
                        .require_path_only()
                        .is_ok_and(|field_attr_path| {
                            field_attr_path.is_ident("from") || field_attr_path.is_ident("source")
                        })
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
            syn::Fields::Unnamed(fields) if !fields.unnamed.is_empty() => Some(0),
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
