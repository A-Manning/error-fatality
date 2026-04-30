use indexmap::IndexMap;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::{
    DataEnum, DataStruct, FieldPat, Fields, ItemEnum, ItemStruct, LitBool, Member, Pat, PatIdent,
    PatPath, PatRest, PatStruct, PatTupleStruct, PatWild, Path, PathArguments, PathSegment, Token,
    Variant,
    parse::{Parse, ParseStream},
    parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Brace, Paren, PathSep},
};

use proc_macro_crate::{FoundCrate, crate_name};

mod split;

/// Similar to [`syn::DeriveInput`], but specialized to a particular data type
#[derive(Clone, Debug)]
pub(crate) struct DeriveInput<Data> {
    pub attrs: Vec<syn::Attribute>,
    pub vis: syn::Visibility,
    pub ident: Ident,
    pub generics: syn::Generics,
    pub data: Data,
}

impl From<syn::DeriveInput> for DeriveInput<syn::Data> {
    fn from(derive_input: syn::DeriveInput) -> Self {
        let syn::DeriveInput {
            attrs,
            vis,
            ident,
            generics,
            data,
        } = derive_input;
        Self {
            attrs,
            vis,
            ident,
            generics,
            data,
        }
    }
}

impl From<DeriveInput<DataEnum>> for ItemEnum {
    fn from(derive_input: DeriveInput<DataEnum>) -> Self {
        Self {
            attrs: derive_input.attrs,
            vis: derive_input.vis,
            enum_token: derive_input.data.enum_token,
            ident: derive_input.ident,
            generics: derive_input.generics,
            brace_token: derive_input.data.brace_token,
            variants: derive_input.data.variants,
        }
    }
}

impl From<DeriveInput<DataStruct>> for ItemStruct {
    fn from(derive_input: DeriveInput<DataStruct>) -> Self {
        Self {
            attrs: derive_input.attrs,
            vis: derive_input.vis,
            struct_token: derive_input.data.struct_token,
            ident: derive_input.ident,
            generics: derive_input.generics,
            semi_token: derive_input.data.semi_token,
            fields: derive_input.data.fields,
        }
    }
}

impl Parse for DeriveInput<syn::Data> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let derive_input = syn::DeriveInput::parse(input)?;
        Ok(derive_input.into())
    }
}

pub(crate) mod kw {
    // Variant fatality is determined based on the inner value, if there is only one, if multiple, the first is chosen.
    syn::custom_keyword!(forward);
    // Scrape the `thiserror` `transparent` annotation.
    syn::custom_keyword!(transparent);
    // Enum annotation to be splitable.
    syn::custom_keyword!(splitable);
    // Expand a particular annotation and only that.
    syn::custom_keyword!(expand);
}

#[derive(Clone, Default)]
pub(crate) enum ResolutionMode {
    /// Not relevant for fatality determination, always non-fatal.
    NoAnnotation,
    /// Fatal by default.
    #[default]
    Fatal,
    /// Specified via a `bool` argument `#[fatal(true)]` or `#[fatal(false)]`.
    WithExplicitBool(LitBool),
    /// Specified via a keyword argument `#[fatal(forward)]`.
    Forward(kw::forward, Option<syn::Member>),
}

impl std::fmt::Debug for ResolutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAnnotation => writeln!(f, "None"),
            Self::Fatal => writeln!(f, "Fatal"),
            Self::WithExplicitBool(b) => writeln!(f, "Fatal({})", b.value()),
            Self::Forward(_, member) => writeln!(
                f,
                "Fatal(Forward, {})",
                member
                    .as_ref()
                    .map(|m| match m {
                        syn::Member::Named(x) => x.to_string(),
                        syn::Member::Unnamed(idx) => idx.index.to_string(),
                    })
                    .unwrap_or_else(|| "___".to_string())
            ),
        }
    }
}

impl Parse for ResolutionMode {
    fn parse(input: ParseStream) -> Result<Self, syn::Error> {
        let lookahead = input.lookahead1();

        if lookahead.peek(kw::forward) {
            Ok(Self::Forward(input.parse::<kw::forward>()?, None))
        } else if lookahead.peek(LitBool) {
            Ok(Self::WithExplicitBool(input.parse::<LitBool>()?))
        } else {
            Err(lookahead.error())
        }
    }
}

impl ToTokens for ResolutionMode {
    fn to_tokens(&self, ts: &mut TokenStream) {
        let trait_fatality = abs_helper_path(format_ident!("Fatality"), Span::call_site());
        let tmp = match self {
            Self::NoAnnotation => quote! { false },
            Self::Fatal => quote! { true },
            Self::WithExplicitBool(boolean) => {
                let value = boolean.value;
                quote! { #value }
            }
            Self::Forward(_, maybe_member) => {
                let ident = match maybe_member
                    .clone()
                    .expect("Forward must have ident set. qed")
                {
                    syn::Member::Named(ident) => ident,
                    syn::Member::Unnamed(idx) => {
                        unnamed_fields_variant_pattern_constructor_binding_name(idx.index as usize)
                    }
                };
                quote! {
                    <_ as #trait_fatality >::is_fatal( #ident )
                }
            }
        };
        ts.extend(tmp)
    }
}

fn abs_helper_path(what: impl Into<Path>, loco: Span) -> Path {
    let what = what.into();
    let found_crate = if cfg!(test) {
        FoundCrate::Itself
    } else {
        crate_name("fatality").expect("`fatality` must be present in `Cargo.toml` for use. q.e.d")
    };
    let path: Path = match found_crate {
        FoundCrate::Itself => parse_quote!( crate::#what ),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, loco);
            parse_quote! { :: #ident :: #what }
        }
    };
    path
}

/// Implement `trait Fatality` for `who`.
fn trait_fatality_impl_for_enum(
    who: &Ident,
    pattern_lut: &IndexMap<Variant, Pat>,
    resolution_lut: &IndexMap<Variant, ResolutionMode>,
) -> TokenStream {
    let pat = pattern_lut.values();
    let resolution = resolution_lut.values();

    let fatality_trait = abs_helper_path(Ident::new("Fatality", who.span()), who.span());
    quote! {
        impl #fatality_trait for #who {
            fn is_fatal(&self) -> bool {
                match self {
                    #( #pat => #resolution, )*
                }
            }
        }
    }
}

/// Implement `trait Fatality` for `who`.
fn trait_fatality_impl_for_struct(who: &Ident, resolution: &ResolutionMode) -> TokenStream {
    let fatality_trait = abs_helper_path(Ident::new("Fatality", who.span()), who.span());
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
        impl #fatality_trait for #who {
            fn is_fatal(&self) -> bool {
                #resolution
            }
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct Transparent(kw::transparent);

impl Parse for Transparent {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();

        if lookahead.peek(kw::transparent) {
            Ok(Self(input.parse::<kw::transparent>()?))
        } else {
            Err(lookahead.error())
        }
    }
}

/// Returns the pattern to match, and if there is an inner ident
/// that was annotated with `#[source]`, which would be used to defer
/// `is_fatal` resolution.
///
/// Consumes a requested `ResolutionMode` and returns the same mode,
/// with a populated identifier, or errors.
fn enum_variant_to_pattern(
    variant: &Variant,
    requested_resolution_mode: ResolutionMode,
) -> Result<(Pat, ResolutionMode), syn::Error> {
    to_pattern(
        &variant.ident,
        &variant.fields,
        &variant.attrs,
        requested_resolution_mode,
    )
}

fn struct_to_pattern(
    item: &DeriveInput<DataStruct>,
    requested_resolution_mode: ResolutionMode,
) -> Result<(Pat, ResolutionMode), syn::Error> {
    to_pattern(
        &item.ident,
        &item.data.fields,
        &item.attrs,
        requested_resolution_mode,
    )
}

fn to_pattern(
    name: &Ident,
    fields: &Fields,
    attrs: &[syn::Attribute],
    requested_resolution_mode: ResolutionMode,
) -> Result<(Pat, ResolutionMode), syn::Error> {
    let span = fields.span();
    // default name for referencing a var in an unnamed enum variant
    let me = PathSegment {
        ident: Ident::new("Self", span),
        arguments: PathArguments::None,
    };
    let path = Path {
        leading_colon: None,
        segments: Punctuated::<PathSegment, PathSep>::from_iter(vec![me, name.clone().into()]),
    };
    let is_transparent = attrs
        .iter()
        .find(|attr| {
            if attr.path().is_ident("error") {
                attr.parse_args::<Transparent>().is_ok()
            } else {
                false
            }
        })
        .is_some();

    let source = Ident::new("source", span);
    let from = Ident::new("from", span);

    let (pat, resolution) = match fields {
        Fields::Named(fields) => {
            let (fields, resolution) = {
                let (fwd_keyword, _ident) = match &requested_resolution_mode {
                    ResolutionMode::NoAnnotation => {
                        let fwd_keyword = kw::forward {
                            span: requested_resolution_mode.span(),
                        };
                        (Some(fwd_keyword), None)
                    }
                    ResolutionMode::Forward(keyword, _ident) => (Some(*keyword), Some(_ident)),
                    _ => (None, None),
                };
                if let Some(fwd_keyword) = fwd_keyword {
                    let fwd_field = if is_transparent {
                        fields.named.first().ok_or_else(|| syn::Error::new(fields.span(), "Missing inner field, must have exactly one inner field type, but requires one for `#[fatal(forward)]`."))?
                    } else {
                        fields.named.iter().find(|field| {
                            field
                                .attrs
                                .iter()
                                .find(|attr| attr.path().is_ident(&source) || attr.path().is_ident(&from))
                                .is_some()
                        })
                        .or_else(|| {
                            fields.named.iter().find(|field| {
                                field
                                    .ident
                                    .as_ref()
                                    .is_some_and(|field_ident| field_ident == "source")
                            })
                        })
                        .ok_or_else(|| syn::Error::new(
                            fields.span(),
                            "No field annotated with `#[source]` or `#[from]`, but requires one for `#[fatal(forward)]`.")
                        )?
                    };
                    if let Some(_ident) = _ident {
                        assert!(_ident.is_none());
                    }
                    // let fwd_field = fwd_field.as_ref().unwrap();
                    let field_name = fwd_field
                        .ident
                        .clone()
                        .expect("Must have member/field name. qed");
                    let fp = FieldPat {
                        attrs: vec![],
                        member: Member::Named(field_name.clone()),
                        colon_token: None,
                        pat: Box::new(Pat::Ident(PatIdent {
                            attrs: vec![],
                            by_ref: None,
                            mutability: None,
                            ident: field_name.clone(),
                            subpat: None,
                        })),
                    };
                    (
                        Punctuated::<FieldPat, Token![,]>::from_iter([fp]),
                        ResolutionMode::Forward(
                            fwd_keyword,
                            fwd_field.ident.clone().map(syn::Member::from),
                        ),
                    )
                } else {
                    (
                        Punctuated::<FieldPat, Token![,]>::new(),
                        requested_resolution_mode,
                    )
                }
            };
            (
                Pat::Struct(PatStruct {
                    attrs: vec![],
                    path,
                    brace_token: Brace(span),
                    fields,
                    qself: None,
                    rest: Some(PatRest {
                        attrs: vec![],
                        dot2_token: Token![..](span),
                    }),
                }),
                resolution,
            )
        }
        Fields::Unnamed(fields) => {
            let (mut field_pats, resolution) = {
                let fwd_keyword = match &requested_resolution_mode {
                    ResolutionMode::NoAnnotation => Some(kw::forward {
                        span: requested_resolution_mode.span(),
                    }),
                    ResolutionMode::Forward(keyword, _ident) => Some(*keyword),
                    _ => None,
                };
                if let Some(fwd_keyword) = fwd_keyword {
                    // obtain the i of the i-th unnamed field.
                    let fwd_idx = if is_transparent {
                        // must be the only field, otherwise bail
                        if fields.unnamed.iter().count() != 1 {
                            return Err(syn::Error::new(
                                fields.span(),
                                "Must have exactly one parameter when annotated with `#[transparent]` annotated field for `forward` with `fatality`",
                            ));
                        }
                        0_usize
                    } else {
                        fields
                            .unnamed
                            .iter()
                            .enumerate()
                            .find_map(|(idx, field)| {
                                field
                                    .attrs
                                    .iter()
                                    .find(|attr| {
                                        attr.path().is_ident(&source) || attr.path().is_ident(&from)
                                    })
                                    .map(|_attr| idx)
                            })
                            .ok_or_else(|| {
                                syn::Error::new(
                                            span,
                                            "Must have a `#[source]` or `#[from]` annotated field for `#[fatal(forward)]`",
                                    )
                            })?
                    };

                    let pat_capture_ident =
                        unnamed_fields_variant_pattern_constructor_binding_name(fwd_idx);
                    // create a pattern like this: `_, _, _, inner, ..`
                    let mut field_pats = std::iter::repeat_n(
                        Pat::Wild(PatWild {
                            attrs: vec![],
                            underscore_token: Token![_](span),
                        }),
                        fwd_idx,
                    )
                    .collect::<Vec<_>>();

                    field_pats.push(Pat::Ident(PatIdent {
                        attrs: vec![],
                        by_ref: None,
                        mutability: None,
                        ident: pat_capture_ident.clone(),
                        subpat: None,
                    }));

                    (
                        field_pats,
                        ResolutionMode::Forward(fwd_keyword, Some(fwd_idx.into())),
                    )
                } else {
                    (vec![], requested_resolution_mode)
                }
            };
            field_pats.push(Pat::Rest(PatRest {
                attrs: vec![],
                dot2_token: Token![..](span),
            }));
            (
                Pat::TupleStruct(PatTupleStruct {
                    attrs: vec![],
                    path,
                    qself: None,
                    paren_token: Paren(span),
                    elems: Punctuated::<Pat, Token![,]>::from_iter(field_pats),
                }),
                resolution,
            )
        }
        Fields::Unit => {
            if let ResolutionMode::Forward(..) = requested_resolution_mode {
                return Err(syn::Error::new(
                    span,
                    "Cannot forward to a unit item variant",
                ));
            }
            (
                Pat::Path(PatPath {
                    attrs: vec![],
                    qself: None,
                    path,
                }),
                requested_resolution_mode,
            )
        }
    };
    assert!(
        !matches!(resolution, ResolutionMode::Forward(_kw, None)),
        "We always set the resolution identifier _right here_. qed"
    );

    Ok((pat, resolution))
}

fn unnamed_fields_variant_pattern_constructor_binding_name(ith: usize) -> Ident {
    Ident::new(format!("arg_{}", ith).as_str(), Span::call_site())
}

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

/// Mutably borrow a field by index
fn get_field_mut(fields: &mut syn::Fields, idx: usize) -> Option<&mut syn::Field> {
    match fields {
        syn::Fields::Named(fields) => fields.named.get_mut(idx),
        syn::Fields::Unnamed(fields) => fields.unnamed.get_mut(idx),
        syn::Fields::Unit => None,
    }
}

/// Generate the Jfyi and Fatal sub enums.
///
/// `fatal_variants` and `jfyi_variants` cover _all_ variants, if they are forward, they are part of both slices.
/// `forward_variants` enlists all variants that
fn trait_split_impl(
    split_opts: split::Opts,
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
        let attrs = if let Some(attrs) = split_opts.attrs.clone() {
            attrs.into_iter().map(outer_attr).collect()
        } else {
            let derive_attr = default_split_derive_attr(Span::call_site());
            let retained_attrs = original
                .attrs
                .iter()
                .filter(|attr| !attr.path().is_ident("split"))
                .cloned();
            std::iter::once(derive_attr).chain(retained_attrs).collect()
        };
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
        let attrs = if let Some(attrs) = split_opts.attrs.clone() {
            attrs.into_iter().map(outer_attr).collect()
        } else {
            let derive_attr = default_split_derive_attr(Span::call_site());
            let retained_attrs = original
                .attrs
                .iter()
                .filter(|attr| !attr.path().is_ident("split"))
                .cloned();
            std::iter::once(derive_attr).chain(retained_attrs).collect()
        };
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
                "Cannot be annotated as fatal when in the JFYI slice. qed"
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

/// Generate the Jfyi and Fatal sub structs.
///
/// `fatal_variants` and `jfyi_variants` cover _all_ variants, if they are forward, they are part of both slices.
/// `forward_variants` enlists all variants that
fn trait_split_struct_impl(
    split_opts: split::Opts,
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
        let attrs = if let Some(attrs) = split_opts.attrs.clone() {
            attrs.into_iter().map(outer_attr).collect()
        } else {
            let derive_attr = default_split_derive_attr(Span::call_site());
            let retained_attrs = original
                .attrs
                .iter()
                .filter(|attr| !attr.path().is_ident("split"))
                .cloned();
            std::iter::once(derive_attr).chain(retained_attrs).collect()
        };
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
        let attrs = if let Some(attrs) = split_opts.attrs {
            attrs.into_iter().map(outer_attr).collect()
        } else {
            let derive_attr = default_split_derive_attr(Span::call_site());
            let retained_attrs = original
                .attrs
                .iter()
                .filter(|attr| !attr.path().is_ident("split"))
                .cloned();
            std::iter::once(derive_attr).chain(retained_attrs).collect()
        };
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

pub(crate) fn fatality_struct_gen(
    mut item: DeriveInput<DataStruct>,
) -> syn::Result<proc_macro2::TokenStream> {
    let mut resolution_mode = ResolutionMode::NoAnnotation;

    // remove the `#[fatal]` attribute
    while let Some(idx) = item.attrs.iter().enumerate().find_map(|(idx, attr)| {
        if attr.path().is_ident("fatal") {
            Some(idx)
        } else {
            None
        }
    }) {
        let attr = item.attrs.remove(idx);
        if attr.meta.require_path_only().is_ok() {
            // no argument to `#[fatal]` means it's fatal
            resolution_mode = ResolutionMode::Fatal;
        } else {
            // parse whatever was passed to `#[fatal(..)]`.
            resolution_mode = attr.parse_args::<ResolutionMode>()?;
        }
    }

    let (_pat, resolution_mode) = struct_to_pattern(&item, resolution_mode)?;

    Ok(trait_fatality_impl_for_struct(
        &item.ident,
        &resolution_mode,
    ))
}

pub(crate) fn split_struct_gen(
    span: proc_macro2::Span,
    mut item: DeriveInput<DataStruct>,
) -> syn::Result<proc_macro2::TokenStream> {
    let split_opts = split::Opts::from_attrs(&item.attrs)?;
    let mut resolution_mode = ResolutionMode::NoAnnotation;

    // remove the `#[fatal]` attribute
    while let Some(idx) = item.attrs.iter().enumerate().find_map(|(idx, attr)| {
        if attr.path().is_ident("fatal") {
            Some(idx)
        } else {
            None
        }
    }) {
        let attr = item.attrs.remove(idx);
        if attr.meta.require_path_only().is_ok() {
            // no argument to `#[fatal]` means it's fatal
            resolution_mode = ResolutionMode::Fatal;
        } else {
            // parse whatever was passed to `#[fatal(..)]`.
            resolution_mode = attr.parse_args::<ResolutionMode>()?;
        }
    }

    let (_pat, resolution_mode) = struct_to_pattern(&item, resolution_mode)?;

    match resolution_mode {
        ResolutionMode::Fatal | ResolutionMode::WithExplicitBool(_) => {
            let err_msg = "cannot specify a fatality for splitable structs";
            return Err(syn::Error::new(span, err_msg));
        }
        ResolutionMode::NoAnnotation => {
            let err_msg = "splitable structs must have a source field";
            return Err(syn::Error::new(span, err_msg));
        }
        ResolutionMode::Forward(_, _) => (),
    }
    if item.data.fields.is_empty() {
        let err_msg = "Cannot derive `Split` for a unit struct";
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
            "Cannot use `splitable` on a `struct` without a source field",
        ));
    };
    trait_split_struct_impl(split_opts, &item, source_field_idx)
}

pub(crate) fn fatality_enum_gen(mut item: DeriveInput<DataEnum>) -> syn::Result<TokenStream> {
    let mut resolution_lut = IndexMap::new();
    let mut pattern_lut = IndexMap::new();

    // if there is not a single fatal annotation, we can just replace `#[fatality]` with `#[derive(::thiserror::Error, Debug)]`
    // without the intermediate type. But impl `trait Fatality` on-top.
    for variant in item.data.variants.iter_mut() {
        let mut resolution_mode = ResolutionMode::NoAnnotation;

        // remove the `#[fatal]` attribute
        while let Some(idx) = variant.attrs.iter().enumerate().find_map(|(idx, attr)| {
            if attr.path().is_ident("fatal") {
                Some(idx)
            } else {
                None
            }
        }) {
            let attr = variant.attrs.remove(idx);
            if attr.meta.require_path_only().is_ok() {
                resolution_mode = ResolutionMode::Fatal;
            } else {
                resolution_mode = attr.parse_args::<ResolutionMode>()?;
            }
        }

        // Obtain the patterns for each variant, and the resolution, which can either
        // be `forward`, `true`, or `false`
        // as used in the `trait Fatality`.
        let (pattern, resolution_mode) = enum_variant_to_pattern(variant, resolution_mode)?;
        if let ResolutionMode::Forward(_, None) = resolution_mode {
            unreachable!("Must have an ident. qed")
        }
        resolution_lut.insert(variant.clone(), resolution_mode);
        pattern_lut.insert(variant.clone(), pattern);
    }

    Ok(trait_fatality_impl_for_enum(
        &item.ident,
        &pattern_lut,
        &resolution_lut,
    ))
}

pub(crate) fn split_enum_gen(mut item: DeriveInput<DataEnum>) -> syn::Result<TokenStream> {
    let opts = split::Opts::from_attrs(&item.attrs)?;
    let mut resolution_lut = IndexMap::new();

    let mut jfyi_variants = Vec::new();
    let mut fatal_variants = Vec::new();

    // if there is not a single fatal annotation, we can just replace `#[fatality]` with `#[derive(::thiserror::Error, Debug)]`
    // without the intermediate type. But impl `trait Fatality` on-top.
    for variant in item.data.variants.iter_mut() {
        let mut resolution_mode = ResolutionMode::NoAnnotation;

        // remove the `#[fatal]` attribute
        while let Some(idx) = variant.attrs.iter().enumerate().find_map(|(idx, attr)| {
            if attr.path().is_ident("fatal") {
                Some(idx)
            } else {
                None
            }
        }) {
            let attr = variant.attrs.remove(idx);
            if attr.meta.require_path_only().is_ok() {
                resolution_mode = ResolutionMode::Fatal;
            } else {
                resolution_mode = attr.parse_args::<ResolutionMode>()?;
            }
        }

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
            ResolutionMode::WithExplicitBool(ref b) if b.value() => {
                fatal_variants.push(variant.clone())
            }
            ResolutionMode::WithExplicitBool(_) => jfyi_variants.push(variant.clone()),
            ResolutionMode::Fatal => fatal_variants.push(variant.clone()),
            ResolutionMode::NoAnnotation => jfyi_variants.push(variant.clone()),
        }
        resolution_lut.insert(variant.clone(), resolution_mode);
    }

    trait_split_impl(opts, item, &resolution_lut, &jfyi_variants, &fatal_variants)
}
