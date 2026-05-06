use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::{
    Attribute, DataEnum, DataStruct, FieldPat, Fields, ItemEnum, ItemStruct,
    LitBool, Member, Pat, PatIdent, PatPath, PatRest, PatStruct,
    PatTupleStruct, PatWild, Path, PathArguments, PathSegment, Token, Variant,
    parse::{Parse, ParseStream},
    parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Brace, Paren, PathSep},
};

use proc_macro_crate::{FoundCrate, crate_name};

pub(crate) mod fatality;
pub(crate) mod split;

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

fn abs_helper_path(what: impl Into<Path>, loco: Span) -> Path {
    let what = what.into();
    let found_crate = if cfg!(test) {
        FoundCrate::Itself
    } else {
        crate_name("fatality")
            .expect("`fatality` must be present in `Cargo.toml` for use. q.e.d")
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

fn unnamed_fields_variant_pattern_constructor_binding_name(
    ith: usize,
) -> Ident {
    Ident::new(format!("arg_{}", ith).as_str(), Span::call_site())
}

#[derive(Clone)]
pub(crate) enum ResolutionMode {
    /// Specified via a keyword argument `#[fatal(forward)]`.
    Forward(kw::forward, Option<syn::Member>),
    /// Specified via a `bool` argument `#[fatal(true)]` or `#[fatal(false)]`.
    WithExplicitBool(LitBool),
}

impl ResolutionMode {
    /// Extract the resolution mode from attrs.
    /// Returns an error if the resolution mode is specified multiple times.
    fn extract(attrs: &mut Vec<Attribute>) -> syn::Result<Option<Self>> {
        let mut fatal_attr_idx = None;
        for (idx, attr) in attrs
            .iter()
            .enumerate()
            .filter(|(_idx, attr)| attr.path().is_ident("fatal"))
        {
            if fatal_attr_idx.is_none() {
                fatal_attr_idx = Some(idx);
            } else {
                let err_msg = "fatality specified multiple times";
                return Err(syn::Error::new(attr.span(), err_msg));
            }
        }
        let Some(fatal_attr_idx) = fatal_attr_idx else {
            return Ok(None);
        };
        let fatal_attr = attrs.remove(fatal_attr_idx);
        let res = fatal_attr.parse_args::<ResolutionMode>()?;
        Ok(Some(res))
    }

    fn extract_from_variant_attrs(variant: &mut Variant) -> syn::Result<Self> {
        ResolutionMode::extract(&mut variant.attrs)?.ok_or_else(|| {
            let err_msg = "missing `#[fatal(_)]` attribute for variant";
            syn::Error::new(variant.span(), err_msg)
        })
    }

    fn extract_from_struct_attrs(
        strukt: &mut DeriveInput<DataStruct>,
    ) -> syn::Result<Self> {
        ResolutionMode::extract(&mut strukt.attrs)?.ok_or_else(|| {
            let err_msg = "missing `#[fatal(_)]` attribute for struct";
            syn::Error::new(strukt.ident.span(), err_msg)
        })
    }
}

impl std::fmt::Debug for ResolutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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
        let trait_fatality =
            abs_helper_path(format_ident!("Fatality"), Span::call_site());
        let tmp = match self {
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
                        unnamed_fields_variant_pattern_constructor_binding_name(
                            idx.index as usize,
                        )
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
        segments: Punctuated::<PathSegment, PathSep>::from_iter(vec![
            me,
            name.clone().into(),
        ]),
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
                let (fwd_keyword, ident) = match &requested_resolution_mode {
                    ResolutionMode::Forward(keyword, ident) => {
                        (Some(*keyword), Some(ident))
                    }
                    ResolutionMode::WithExplicitBool(_) => (None, None),
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
                    if let Some(ident) = ident {
                        assert!(ident.is_none());
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
                    ResolutionMode::Forward(keyword, _ident) => Some(*keyword),
                    ResolutionMode::WithExplicitBool(_) => None,
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
                        unnamed_fields_variant_pattern_constructor_binding_name(
                            fwd_idx,
                        );
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
                        ResolutionMode::Forward(
                            fwd_keyword,
                            Some(fwd_idx.into()),
                        ),
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
                    "cannot forward to a unit item variant",
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
