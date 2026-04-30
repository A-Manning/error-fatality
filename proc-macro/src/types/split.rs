use syn::{Meta, Token, punctuated::Punctuated, spanned::Spanned};

/// Options provided via the `#[split(_)]` attribute
#[derive(Clone, Debug)]
pub(in crate::types) struct Opts {
    pub attrs: Option<Punctuated<Meta, Token![,]>>,
}

impl Opts {
    const INVALID_ATTR_PATH_ERR_MSG: &str = "invalid attribute path";

    const INVALID_SYNTAX_ERR_MSG: &str = "invalid syntax for `split` attribute";

    const MULTIPLE_ATTRS_ERR_MSG: &str = "cannot set attrs multiple times";

    /// Parse from a single attribute. Returns an error if the attribute path
    /// does not match.
    fn from_attr(attr: &syn::Attribute) -> syn::Result<Self> {
        let mut res = Self { attrs: None };
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
                    if res.attrs.is_none() {
                        res.attrs = Some(
                            meta.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?,
                        );
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
        let Self { attrs: l_attrs } = self;
        let Self { attrs: r_attrs } = other;
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
        Ok(Self { attrs })
    }

    pub(crate) fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
        let mut res = Self { attrs: None };
        for attr in attrs {
            if !attr.path().is_ident("split") {
                continue;
            }
            res = res.extend(Self::from_attr(attr)?)?;
        }
        Ok(res)
    }
}
