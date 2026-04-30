mod fatality;
mod split;

mod component {
    use assert_matches::assert_matches;
    use quote::quote;

    use crate::ResolutionMode;

    #[test]
    fn parse_attr_resmode_forward() {
        let input = quote! { forward };
        let result = syn::parse2::<ResolutionMode>(input).unwrap();
        assert_matches!(result, ResolutionMode::Forward(..));
    }

    #[test]
    fn parse_full_attr() {
        let tokens = quote! { #[fatal(forward)] };
        let mut input = syn::parse::Parser::parse2(syn::Attribute::parse_outer, tokens).unwrap();
        let attr = input.pop().unwrap();
        let result = attr.parse_args::<ResolutionMode>();
        assert_matches!(result, Ok(ResolutionMode::Forward(..)));
    }
}
