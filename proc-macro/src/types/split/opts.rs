//! Options provided via the `#[split(_)]` attribute

use proc_macro2::Span;
use quote::ToTokens;
use syn::{
    Attribute, LitStr, Meta, PathArguments, Token, parse::ParseStream,
    punctuated::Punctuated, spanned::Spanned, token::PathSep,
};

use crate::split::SplitVariant;

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
fn derive_attr(
    span: Span,
    derives: Punctuated<syn::Meta, syn::Token![,]>,
) -> syn::Attribute {
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
struct Attrs(Punctuated<Meta, Token![,]>);

impl ToTokens for Attrs {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens)
    }
}

enum OptsInnerPath {
    /// `#[split(fatal(_))]`
    SplitFatal,
    /// `#[split(jfyi(_))]`
    SplitJfyi,
}

impl OptsInnerPath {
    const fn as_attr(&self) -> &str {
        match self {
            Self::SplitFatal => "split(fatal(_))",
            Self::SplitJfyi => "split(jfyi(_))",
        }
    }
}

enum OptsPath {
    /// `#[split(fatal(_))]` / `#[split(jfyi(_))]`
    Inner(OptsInnerPath),
    /// `#[split(_)]`
    Split,
}

impl OptsPath {
    const fn as_attr(&self) -> &str {
        match self {
            Self::Inner(inner) => inner.as_attr(),
            Self::Split => "split(_)",
        }
    }
}

/// Options provided via the `#[split(_)]`, `#[split(fatal(_))]`, or
/// `#[split(jfyi(_))]` attributes
#[derive(Clone, Debug, Default)]
struct OptsInner {
    attrs: Option<Attrs>,
    ident: Option<LitStr>,
}

impl OptsInner {
    const MULTIPLE_ATTRS_ERR_MSG: &str = "cannot set attrs multiple times";
    const MULTIPLE_IDENT_ERR_MSG: &str = "cannot set ident multiple times";

    fn invalid_syntax_err_msg(path: &OptsPath) -> String {
        format!("invalid syntax for `{}` attribute", path.as_attr())
    }

    fn parse_meta_list(
        &mut self,
        path: &OptsPath,
        meta: syn::MetaList,
    ) -> syn::Result<()> {
        let Self { attrs, ident: _ } = self;
        if matches!(path, OptsPath::Split)
            && (meta.path.is_ident("fatal") || meta.path.is_ident("jfyi"))
        {
            Ok(())
        } else if meta.path.is_ident("attrs") {
            if attrs.is_none() {
                *attrs = Some(Attrs(meta.parse_args_with(
                    Punctuated::<Meta, Token![,]>::parse_terminated,
                )?));
                Ok(())
            } else {
                Err(syn::Error::new(meta.span(), Self::MULTIPLE_ATTRS_ERR_MSG))
            }
        } else {
            Err(syn::Error::new(
                meta.span(),
                Self::invalid_syntax_err_msg(path),
            ))
        }
    }

    fn parse_meta_name_value(
        &mut self,
        path: &OptsPath,
        meta: syn::MetaNameValue,
    ) -> syn::Result<()> {
        let Self { attrs: _, ident } = self;
        let invalid_syntax_err_msg =
            || syn::Error::new(meta.span(), Self::invalid_syntax_err_msg(path));
        if !matches!(path, OptsPath::Inner(_)) {
            return Err(invalid_syntax_err_msg());
        }
        if meta.path.is_ident("ident") {
            if ident.is_none() {
                let syn::Expr::Lit(syn::ExprLit {
                    attrs: _,
                    lit: syn::Lit::Str(ident_str),
                }) = meta.value
                else {
                    return Err(invalid_syntax_err_msg());
                };
                *ident = Some(ident_str);
                Ok(())
            } else {
                Err(syn::Error::new(meta.span(), Self::MULTIPLE_IDENT_ERR_MSG))
            }
        } else {
            Err(invalid_syntax_err_msg())
        }
    }

    fn parse_meta(
        &mut self,
        path: &OptsPath,
        meta: syn::Meta,
    ) -> syn::Result<()> {
        match meta {
            Meta::List(meta) => self.parse_meta_list(path, meta),
            Meta::NameValue(meta) => self.parse_meta_name_value(path, meta),
            Meta::Path(_) => Err(syn::Error::new(
                meta.span(),
                Self::invalid_syntax_err_msg(path),
            )),
        }
    }

    fn parse(path: OptsPath) -> impl FnOnce(ParseStream) -> syn::Result<Self> {
        move |input| {
            let mut res = Self::default();
            let nested =
                Punctuated::<Meta, Token![,]>::parse_terminated(input)?;
            if nested.is_empty() {
                return Err(syn::Error::new(
                    nested.span(),
                    Self::invalid_syntax_err_msg(&path),
                ));
            }
            for meta in nested {
                let () = res.parse_meta(&path, meta)?;
            }
            Ok(res)
        }
    }
}

/// Option with specified target(s) (fatal, jfyi, either, or both)
#[derive(Clone, Debug)]
enum TargetedOpt<Opt> {
    Both { fatal: Opt, jfyi: Opt },
    Either(Opt),
    Fatal(Opt),
    Jfyi(Opt),
}

impl<Opt> TargetedOpt<Opt> {
    fn as_ref(&self) -> TargetedOpt<&Opt> {
        match self {
            Self::Both { fatal, jfyi } => TargetedOpt::Both { fatal, jfyi },
            Self::Either(opt) => TargetedOpt::Either(opt),
            Self::Fatal(opt) => TargetedOpt::Fatal(opt),
            Self::Jfyi(opt) => TargetedOpt::Jfyi(opt),
        }
    }

    fn extract(self, variant: SplitVariant) -> Option<Opt> {
        match (variant, self) {
            (
                SplitVariant::Fatal,
                Self::Both { fatal, jfyi: _ }
                | Self::Either(fatal)
                | Self::Fatal(fatal),
            ) => Some(fatal),
            (SplitVariant::Fatal, Self::Jfyi(_)) => None,
            (
                SplitVariant::Jfyi,
                Self::Both { fatal: _, jfyi }
                | Self::Either(jfyi)
                | Self::Jfyi(jfyi),
            ) => Some(jfyi),
            (SplitVariant::Jfyi, Self::Fatal(_)) => None,
        }
    }
}

impl<Opt> TargetedOpt<Opt>
where
    Opt: Spanned,
{
    const MULTIPLE_OPT_ERR_MSG: &str = "cannot set split option multiple times";

    fn merge(self, other: Self) -> syn::Result<Self> {
        match (self, other) {
            (
                Self::Both { .. },
                Self::Both {
                    fatal: opt,
                    jfyi: _,
                }
                | Self::Either(opt)
                | Self::Fatal(opt)
                | Self::Jfyi(opt),
            )
            | (
                Self::Either(_) | Self::Fatal(_),
                Self::Both {
                    fatal: opt,
                    jfyi: _,
                },
            )
            | (
                Self::Jfyi(_),
                Self::Both {
                    fatal: _,
                    jfyi: opt,
                },
            )
            | (Self::Either(_), Self::Either(opt))
            | (Self::Fatal(_), Self::Fatal(opt))
            | (Self::Jfyi(_), Self::Jfyi(opt)) => {
                Err(syn::Error::new(opt.span(), Self::MULTIPLE_OPT_ERR_MSG))
            }
            (Self::Either(fatal), Self::Jfyi(jfyi))
            | (Self::Either(jfyi), Self::Fatal(fatal))
            | (Self::Fatal(fatal), Self::Either(jfyi))
            | (Self::Fatal(fatal), Self::Jfyi(jfyi))
            | (Self::Jfyi(jfyi), Self::Either(fatal))
            | (Self::Jfyi(jfyi), Self::Fatal(fatal)) => {
                Ok(Self::Both { fatal, jfyi })
            }
        }
    }
}

/// Options provided via the `#[split(_)]` attribute
#[derive(Clone, Debug)]
pub(in crate::types::split) struct Opts {
    attrs: Option<TargetedOpt<Attrs>>,
    ident: Option<TargetedOpt<LitStr>>,
}

impl Opts {
    const INVALID_ATTR_PATH_ERR_MSG: &str = "invalid attribute path";

    const INVALID_SYNTAX_ERR_MSG: &str = "invalid syntax for `split` attribute";

    /// Parse from a single attribute. Returns an error if the attribute path
    /// does not match.
    fn from_attr(attr: &syn::Attribute) -> syn::Result<Self> {
        let mut res = Self {
            attrs: None,
            ident: None,
        };

        if !attr.path().is_ident("split") {
            return Err(syn::Error::new(
                attr.span(),
                Self::INVALID_ATTR_PATH_ERR_MSG,
            ));
        };
        let nested = attr
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        if nested.is_empty() {
            return Err(syn::Error::new(
                nested.span(),
                Self::INVALID_SYNTAX_ERR_MSG,
            ));
        }
        for meta in nested {
            match meta {
                Meta::List(meta) if meta.path.is_ident("fatal") => {
                    let OptsInner {
                        attrs: fatal_attrs,
                        ident: fatal_ident,
                    } = meta.parse_args_with(OptsInner::parse(
                        OptsPath::Inner(OptsInnerPath::SplitFatal),
                    ))?;
                    if let Some(fatal_attrs) = fatal_attrs {
                        let fatal_attrs = TargetedOpt::Fatal(fatal_attrs);
                        res.attrs = if let Some(attrs) = res.attrs.take() {
                            Some(attrs.merge(fatal_attrs)?)
                        } else {
                            Some(fatal_attrs)
                        }
                    }
                    if let Some(fatal_ident) = fatal_ident {
                        let fatal_ident = TargetedOpt::Fatal(fatal_ident);
                        res.ident = if let Some(ident) = res.ident.take() {
                            Some(ident.merge(fatal_ident)?)
                        } else {
                            Some(fatal_ident)
                        }
                    }
                }
                Meta::List(meta) if meta.path.is_ident("jfyi") => {
                    let OptsInner {
                        attrs: jfyi_attrs,
                        ident: jfyi_ident,
                    } = meta.parse_args_with(OptsInner::parse(
                        OptsPath::Inner(OptsInnerPath::SplitJfyi),
                    ))?;
                    if let Some(jfyi_attrs) = jfyi_attrs {
                        let jfyi_attrs = TargetedOpt::Jfyi(jfyi_attrs);
                        res.attrs = if let Some(attrs) = res.attrs.take() {
                            Some(attrs.merge(jfyi_attrs)?)
                        } else {
                            Some(jfyi_attrs)
                        }
                    }
                    if let Some(jfyi_ident) = jfyi_ident {
                        let jfyi_ident = TargetedOpt::Jfyi(jfyi_ident);
                        res.ident = if let Some(ident) = res.ident.take() {
                            Some(ident.merge(jfyi_ident)?)
                        } else {
                            Some(jfyi_ident)
                        }
                    }
                }
                Meta::List(meta) => {
                    let OptsInner {
                        attrs: either_attrs,
                        ident: _,
                    } = {
                        let mut opts = OptsInner::default();
                        opts.parse_meta_list(&OptsPath::Split, meta)?;
                        opts
                    };
                    if let Some(either_attrs) = either_attrs {
                        let either_attrs = TargetedOpt::Either(either_attrs);
                        res.attrs = if let Some(attrs) = res.attrs.take() {
                            Some(attrs.merge(either_attrs)?)
                        } else {
                            Some(either_attrs)
                        }
                    }
                }
                Meta::NameValue(_) | Meta::Path(_) => {
                    return Err(syn::Error::new(
                        meta.span(),
                        Self::INVALID_SYNTAX_ERR_MSG,
                    ));
                }
            }
        }
        Ok(res)
    }

    fn extend(self, other: Self) -> syn::Result<Self> {
        let Self {
            attrs: l_attrs,
            ident: l_ident,
        } = self;
        let Self {
            attrs: r_attrs,
            ident: r_ident,
        } = other;
        let attrs = match (l_attrs, r_attrs) {
            (l_attrs, None) => l_attrs,
            (None, Some(r_attrs)) => Some(r_attrs),
            (Some(l_attrs), Some(r_attrs)) => Some(l_attrs.merge(r_attrs)?),
        };
        let ident = match (l_ident, r_ident) {
            (l_ident, None) => l_ident,
            (None, Some(r_ident)) => Some(r_ident),
            (Some(l_ident), Some(r_ident)) => Some(l_ident.merge(r_ident)?),
        };
        Ok(Self { attrs, ident })
    }

    pub(crate) fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
        let mut res = Self {
            attrs: None,
            ident: None,
        };
        for attr in attrs {
            if !attr.path().is_ident("split") {
                continue;
            }
            res = res.extend(Self::from_attr(attr)?)?;
        }
        Ok(res)
    }

    pub(in crate::types::split) fn split_error_attrs(
        &self,
        span: Span,
        original_attrs: &[Attribute],
        variant: SplitVariant,
    ) -> Vec<Attribute> {
        let attrs = self
            .attrs
            .as_ref()
            .and_then(|attrs| attrs.as_ref().extract(variant));
        let Some(attrs) = attrs else {
            let derive_attr = default_split_derive_attr(span);
            let retained_attrs = original_attrs
                .iter()
                .filter(|attr| !attr.path().is_ident("split"))
                .cloned();
            return std::iter::once(derive_attr)
                .chain(retained_attrs)
                .collect();
        };
        attrs.0.iter().cloned().map(outer_attr).collect()
    }

    pub(in crate::types::split) fn ident(
        &self,
        variant: SplitVariant,
    ) -> Option<String> {
        self.ident
            .as_ref()
            .and_then(|ident| ident.as_ref().extract(variant))
            .map(|ident| ident.value())
    }
}
