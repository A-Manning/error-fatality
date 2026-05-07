//! Options provided via the `#[split(_)]` attribute

use proc_macro2::Span;
use quote::ToTokens;
use syn::{
    Attribute, GenericParam, LitStr, Meta, PathArguments, Token,
    WherePredicate, parse::ParseStream, punctuated::Punctuated,
    spanned::Spanned, token::PathSep,
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

fn multiple_opt_error(span: Span, opt_name: &str) -> syn::Error {
    let err_msg = format!("cannot set {opt_name} multiple times");
    syn::Error::new(span, err_msg)
}

/// Option with unspecified target
#[derive(Debug)]
#[repr(transparent)]
struct UntargetedOpt<Opt>(Opt);

impl<Opt> UntargetedOpt<Opt> {
    fn as_ref(&self) -> UntargetedOpt<&Opt> {
        let Self(opt) = self;
        UntargetedOpt(opt)
    }

    fn into_inner(self) -> Opt {
        let Self(opt) = self;
        opt
    }
}

impl<Opt> UntargetedOpt<Opt>
where
    Opt: Spanned,
{
    fn span(&self) -> Span {
        let Self(opt) = self;
        opt.span()
    }
}

/// Option with specified target(s) (fatal, jfyi, or both)
#[derive(Debug)]
enum TargetedOpt<Opt> {
    Both { fatal: Opt, jfyi: Opt },
    Fatal(Opt),
    Jfyi(Opt),
}

impl<Opt> TargetedOpt<Opt> {
    fn as_ref(&self) -> TargetedOpt<&Opt> {
        match self {
            Self::Both { fatal, jfyi } => TargetedOpt::Both { fatal, jfyi },
            Self::Fatal(opt) => TargetedOpt::Fatal(opt),
            Self::Jfyi(opt) => TargetedOpt::Jfyi(opt),
        }
    }

    fn new_variant(variant: SplitVariant, opt: Opt) -> Self {
        match variant {
            SplitVariant::Fatal => Self::Fatal(opt),
            SplitVariant::Jfyi => Self::Jfyi(opt),
        }
    }

    fn extract(self, variant: SplitVariant) -> Option<Opt> {
        match (variant, self) {
            (
                SplitVariant::Fatal,
                Self::Both { fatal, jfyi: _ } | Self::Fatal(fatal),
            ) => Some(fatal),
            (SplitVariant::Fatal, Self::Jfyi(_)) => None,
            (
                SplitVariant::Jfyi,
                Self::Both { fatal: _, jfyi } | Self::Jfyi(jfyi),
            ) => Some(jfyi),
            (SplitVariant::Jfyi, Self::Fatal(_)) => None,
        }
    }
}

impl<Opt> TargetedOpt<Opt>
where
    Opt: Spanned,
{
    fn merge(self, other: Self, opt_name: &str) -> syn::Result<Self> {
        match (self, other) {
            (Self::Fatal(fatal), Self::Jfyi(jfyi))
            | (Self::Jfyi(jfyi), Self::Fatal(fatal)) => {
                Ok(Self::Both { fatal, jfyi })
            }
            (
                Self::Both { .. },
                Self::Both {
                    fatal: opt,
                    jfyi: _,
                }
                | Self::Fatal(opt)
                | Self::Jfyi(opt),
            )
            | (
                Self::Fatal(_),
                Self::Both {
                    fatal: opt,
                    jfyi: _,
                }
                | Self::Fatal(opt),
            )
            | (
                Self::Jfyi(_),
                Self::Both {
                    fatal: _,
                    jfyi: opt,
                }
                | Self::Jfyi(opt),
            ) => Err(multiple_opt_error(opt.span(), opt_name)),
        }
    }

    fn merge_untargeted(
        self,
        other: UntargetedOpt<Opt>,
        opt_name: &str,
    ) -> syn::Result<Self> {
        match (self, other) {
            (Self::Fatal(fatal), UntargetedOpt(jfyi))
            | (Self::Jfyi(jfyi), UntargetedOpt(fatal)) => {
                Ok(Self::Both { fatal, jfyi })
            }
            (Self::Both { .. }, UntargetedOpt(opt)) => {
                Err(multiple_opt_error(opt.span(), opt_name))
            }
        }
    }
}

/// Option with possibly specified target(s) (fatal, jfyi, either, or both)
#[derive(Debug)]
enum MaybeTargetedOpt<Opt> {
    Targeted(TargetedOpt<Opt>),
    Untargeted(UntargetedOpt<Opt>),
}

impl<Opt> MaybeTargetedOpt<Opt> {
    fn as_ref(&self) -> MaybeTargetedOpt<&Opt> {
        match self {
            Self::Targeted(opt) => MaybeTargetedOpt::Targeted(opt.as_ref()),
            Self::Untargeted(opt) => MaybeTargetedOpt::Untargeted(opt.as_ref()),
        }
    }

    fn new_variant(variant: SplitVariant, opt: Opt) -> Self {
        Self::Targeted(TargetedOpt::new_variant(variant, opt))
    }

    fn new_untargeted(opt: Opt) -> Self {
        Self::Untargeted(UntargetedOpt(opt))
    }

    fn extract(self, variant: SplitVariant) -> Option<Opt> {
        match self {
            Self::Targeted(opt) => opt.extract(variant),
            Self::Untargeted(opt) => Some(opt.into_inner()),
        }
    }
}

impl<Opt> MaybeTargetedOpt<Opt>
where
    Opt: Spanned,
{
    fn merge(
        self,
        other: Self,
        opt_name: &str,
    ) -> syn::Result<TargetedOpt<Opt>> {
        match (self, other) {
            (Self::Targeted(l_opt), Self::Targeted(r_opt)) => {
                l_opt.merge(r_opt, opt_name)
            }
            (Self::Untargeted(_), Self::Untargeted(r_opt)) => {
                Err(multiple_opt_error(r_opt.span(), opt_name))
            }
            (Self::Targeted(targeted), Self::Untargeted(untargeted))
            | (Self::Untargeted(untargeted), Self::Targeted(targeted)) => {
                targeted.merge_untargeted(untargeted, opt_name)
            }
        }
    }
}

#[derive(Clone, Debug)]
#[repr(transparent)]
struct Attrs(Punctuated<Meta, Token![,]>);

impl ToTokens for Attrs {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens)
    }
}

#[derive(Debug)]
#[repr(transparent)]
struct Bound(Punctuated<WherePredicate, Token![,]>);

impl ToTokens for Bound {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens)
    }
}

#[derive(Debug)]
#[repr(transparent)]
struct Generics(Punctuated<GenericParam, Token![,]>);

impl ToTokens for Generics {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens)
    }
}

enum TargetedOptsPath {
    /// `#[split(fatal(_))]`
    SplitFatal,
    /// `#[split(jfyi(_))]`
    SplitJfyi,
}

impl TargetedOptsPath {
    const fn as_attr(&self) -> &str {
        match self {
            Self::SplitFatal => "split(fatal(_))",
            Self::SplitJfyi => "split(jfyi(_))",
        }
    }

    fn invalid_syntax_error(&self, span: Span) -> syn::Error {
        let err_msg =
            format!("invalid syntax for `{}` attribute", self.as_attr());
        syn::Error::new(span, err_msg)
    }
}

/// `#[split(_)]`
struct UntargetedOptsPath;

impl UntargetedOptsPath {
    fn invalid_syntax_error(span: Span) -> syn::Error {
        const ERR_MSG: &str = "invalid syntax for `split(_)` attribute";
        syn::Error::new(span, ERR_MSG)
    }
}

/// Options provided exclusively via the `#[split(fatal(_))]` or
/// `#[split(jfyi(_))]` attributes
#[derive(Debug, Default)]
struct ExclusivelyTargetedOpts {
    ident: Option<LitStr>,
}

impl ExclusivelyTargetedOpts {
    /// `true` if successfully parsed, `false` if ignored
    fn parse_meta_name_value(
        &mut self,
        path: &TargetedOptsPath,
        meta: &syn::MetaNameValue,
    ) -> syn::Result<bool> {
        let Self { ident } = self;
        if meta.path.is_ident("ident") {
            if ident.is_none() {
                let syn::Expr::Lit(syn::ExprLit {
                    attrs: _,
                    lit: syn::Lit::Str(ident_str),
                }) = &meta.value
                else {
                    return Err(path.invalid_syntax_error(meta.span()));
                };
                *ident = Some(ident_str.clone());
                Ok(true)
            } else {
                Err(multiple_opt_error(meta.span(), "ident"))
            }
        } else {
            Ok(false)
        }
    }

    /// `true` if successfully parsed, `false` if ignored
    fn parse_meta(
        &mut self,
        path: &TargetedOptsPath,
        meta: &syn::Meta,
    ) -> syn::Result<bool> {
        match meta {
            Meta::NameValue(meta) => self.parse_meta_name_value(path, meta),
            Meta::List(_) | Meta::Path(_) => Ok(false),
        }
    }
}

/// Options provided exclusively via the `#[split(_)]` attribute
#[derive(Debug, Default)]
struct ExclusivelyUntargetedOpts {}

impl ExclusivelyUntargetedOpts {
    /// `true` if successfully parsed, `false` if ignored
    fn parse_meta_list(&mut self, _meta: &syn::MetaList) -> syn::Result<bool> {
        Ok(false)
    }
}

/// Options provided via the `#[split(_)]`, `#[split(fatal(_))]`, or
/// `#[split(jfyi(_))]` attributes
#[derive(Debug, Default)]
struct MaybeTargetedOpts {
    attrs: Option<Attrs>,
    bound: Option<Bound>,
    generics: Option<Generics>,
}

impl MaybeTargetedOpts {
    /// `true` if the meta list was successfully parsed, `false` if ignored
    fn parse_meta_list(&mut self, meta: &syn::MetaList) -> syn::Result<bool> {
        let Self {
            attrs,
            bound,
            generics,
        } = self;
        if meta.path.is_ident("attrs") {
            if attrs.is_none() {
                *attrs = Some(Attrs(meta.parse_args_with(
                    Punctuated::<Meta, Token![,]>::parse_terminated,
                )?));
                Ok(true)
            } else {
                Err(multiple_opt_error(meta.span(), "attrs"))
            }
        } else if meta.path.is_ident("bound") {
            if bound.is_none() {
                *bound = Some(Bound(meta.parse_args_with(
                    Punctuated::<WherePredicate, Token![,]>::parse_terminated,
                )?));
                Ok(true)
            } else {
                Err(multiple_opt_error(meta.span(), "bound"))
            }
        } else if meta.path.is_ident("generics") {
            if generics.is_none() {
                *generics = Some(Generics(meta.parse_args_with(
                    Punctuated::<GenericParam, Token![,]>::parse_terminated,
                )?));
                Ok(true)
            } else {
                Err(multiple_opt_error(meta.span(), "generics"))
            }
        } else {
            Ok(false)
        }
    }

    /// `true` if successfully parsed, `false` if ignored
    fn parse_meta(&mut self, meta: &syn::Meta) -> syn::Result<bool> {
        match meta {
            Meta::List(meta) => self.parse_meta_list(meta),
            Meta::NameValue(_) | Meta::Path(_) => Ok(false),
        }
    }
}

/// Options provided via the `#[split(fatal(_))]` or `#[split(jfyi(_))]`
/// attributes
#[derive(Debug, Default)]
struct TargetedOpts {
    exclusive: ExclusivelyTargetedOpts,
    others: MaybeTargetedOpts,
}

impl TargetedOpts {
    fn parse_meta(
        &mut self,
        path: &TargetedOptsPath,
        meta: &syn::Meta,
    ) -> syn::Result<()> {
        let Self { exclusive, others } = self;
        if exclusive.parse_meta(path, meta)? || others.parse_meta(meta)? {
            Ok(())
        } else {
            Err(path.invalid_syntax_error(meta.span()))
        }
    }

    fn parse(
        path: TargetedOptsPath,
    ) -> impl FnOnce(ParseStream) -> syn::Result<Self> {
        move |input| {
            let mut res = Self::default();
            let nested =
                Punctuated::<Meta, Token![,]>::parse_terminated(input)?;
            if nested.is_empty() {
                return Err(path.invalid_syntax_error(nested.span()));
            }
            for meta in nested {
                let () = res.parse_meta(&path, &meta)?;
            }
            Ok(res)
        }
    }
}

/// Options provided via the `#[split(_)]` attribute
#[derive(Debug, Default)]
struct UntargetedOpts {
    exclusive: ExclusivelyUntargetedOpts,
    others: MaybeTargetedOpts,
}

impl UntargetedOpts {
    fn parse_meta_list(&mut self, meta: &syn::MetaList) -> syn::Result<()> {
        let Self { exclusive, others } = self;
        if exclusive.parse_meta_list(meta)? || others.parse_meta_list(meta)? {
            Ok(())
        } else {
            Err(UntargetedOptsPath::invalid_syntax_error(meta.span()))
        }
    }
}

/// Options provided via the `#[split(_)]` attribute
#[derive(Debug, Default)]
pub(in crate::types::split) struct Opts {
    attrs: Option<MaybeTargetedOpt<Attrs>>,
    bound: Option<MaybeTargetedOpt<Bound>>,
    generics: Option<MaybeTargetedOpt<Generics>>,
    ident: Option<TargetedOpt<LitStr>>,
}

impl Opts {
    fn invalid_attr_path_error(span: Span) -> syn::Error {
        const ERR_MSG: &str = "invalid attribute path";
        syn::Error::new(span, ERR_MSG)
    }

    fn invalid_syntax_error(span: Span) -> syn::Error {
        const ERR_MSG: &str = "invalid syntax for `split` attribute";
        syn::Error::new(span, ERR_MSG)
    }

    fn from_targeted(targeted: TargetedOpts, variant: SplitVariant) -> Self {
        let TargetedOpts { exclusive, others } = targeted;
        let ExclusivelyTargetedOpts { ident } = exclusive;
        let MaybeTargetedOpts {
            attrs,
            bound,
            generics,
        } = others;
        Self {
            attrs: attrs
                .map(|attrs| MaybeTargetedOpt::new_variant(variant, attrs)),
            bound: bound
                .map(|bound| MaybeTargetedOpt::new_variant(variant, bound)),
            generics: generics.map(|generics| {
                MaybeTargetedOpt::new_variant(variant, generics)
            }),
            ident: ident.map(|ident| TargetedOpt::new_variant(variant, ident)),
        }
    }

    fn from_untargeted(untargeted: UntargetedOpts) -> Self {
        let UntargetedOpts { exclusive, others } = untargeted;
        let ExclusivelyUntargetedOpts {} = exclusive;
        let MaybeTargetedOpts {
            attrs,
            bound,
            generics,
        } = others;
        Self {
            attrs: attrs.map(MaybeTargetedOpt::new_untargeted),
            bound: bound.map(MaybeTargetedOpt::new_untargeted),
            generics: generics.map(MaybeTargetedOpt::new_untargeted),
            ident: None,
        }
    }

    fn extend(self, other: Self) -> syn::Result<Self> {
        fn merge_maybe_targeted_opts<Opt>(
            opt_name: &str,
            l_opt: Option<MaybeTargetedOpt<Opt>>,
            r_opt: Option<MaybeTargetedOpt<Opt>>,
        ) -> syn::Result<Option<MaybeTargetedOpt<Opt>>>
        where
            Opt: quote::ToTokens,
        {
            match (l_opt, r_opt) {
                (l_opt, None) => Ok(l_opt),
                (None, Some(r_opt)) => Ok(Some(r_opt)),
                (Some(l_opt), Some(r_opt)) => {
                    let merged = l_opt.merge(r_opt, opt_name)?;
                    Ok(Some(MaybeTargetedOpt::Targeted(merged)))
                }
            }
        }
        fn merge_targeted_opts<Opt>(
            opt_name: &str,
            l_opt: Option<TargetedOpt<Opt>>,
            r_opt: Option<TargetedOpt<Opt>>,
        ) -> syn::Result<Option<TargetedOpt<Opt>>>
        where
            Opt: quote::ToTokens,
        {
            match (l_opt, r_opt) {
                (l_opt, None) => Ok(l_opt),
                (None, Some(r_opt)) => Ok(Some(r_opt)),
                (Some(l_opt), Some(r_opt)) => {
                    Ok(Some(l_opt.merge(r_opt, opt_name)?))
                }
            }
        }
        let Self {
            attrs: l_attrs,
            bound: l_bound,
            generics: l_generics,
            ident: l_ident,
        } = self;
        let Self {
            attrs: r_attrs,
            bound: r_bound,
            generics: r_generics,
            ident: r_ident,
        } = other;
        Ok(Self {
            attrs: merge_maybe_targeted_opts("attrs", l_attrs, r_attrs)?,
            bound: merge_maybe_targeted_opts("bound", l_bound, r_bound)?,
            generics: merge_maybe_targeted_opts(
                "generics", l_generics, r_generics,
            )?,
            ident: merge_targeted_opts("ident", l_ident, r_ident)?,
        })
    }

    /// Parse from a single attribute. Returns an error if the attribute path
    /// does not match.
    fn from_attr(attr: &syn::Attribute) -> syn::Result<Self> {
        let mut res = Self::default();
        if !attr.path().is_ident("split") {
            return Err(Self::invalid_attr_path_error(attr.span()));
        };
        let nested = attr
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        if nested.is_empty() {
            return Err(Self::invalid_syntax_error(nested.span()));
        }
        for meta in nested {
            match meta {
                Meta::List(meta) if meta.path.is_ident("fatal") => {
                    let fatal_opts = meta.parse_args_with(
                        TargetedOpts::parse(TargetedOptsPath::SplitFatal),
                    )?;
                    res = res.extend(Self::from_targeted(
                        fatal_opts,
                        SplitVariant::Fatal,
                    ))?;
                }
                Meta::List(meta) if meta.path.is_ident("jfyi") => {
                    let jfyi_opts = meta.parse_args_with(
                        TargetedOpts::parse(TargetedOptsPath::SplitJfyi),
                    )?;
                    res = res.extend(Self::from_targeted(
                        jfyi_opts,
                        SplitVariant::Jfyi,
                    ))?;
                }
                Meta::List(meta) => {
                    let untargeted_opts = {
                        let mut opts = UntargetedOpts::default();
                        opts.parse_meta_list(&meta)?;
                        opts
                    };
                    res = res.extend(Self::from_untargeted(untargeted_opts))?;
                }
                Meta::NameValue(_) | Meta::Path(_) => {
                    return Err(UntargetedOptsPath::invalid_syntax_error(
                        meta.span(),
                    ));
                }
            }
        }
        Ok(res)
    }

    pub(crate) fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
        let mut res = Self::default();
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

    pub(in crate::types::split) fn bound(
        &self,
        variant: SplitVariant,
    ) -> Option<&Punctuated<WherePredicate, Token![,]>> {
        self.bound
            .as_ref()
            .and_then(|bound| bound.as_ref().extract(variant))
            .map(|bound| &bound.0)
    }

    pub(in crate::types::split) fn generics(
        &self,
        variant: SplitVariant,
    ) -> Option<&Punctuated<GenericParam, Token![,]>> {
        self.generics
            .as_ref()
            .and_then(|generics| generics.as_ref().extract(variant))
            .map(|generics| &generics.0)
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
