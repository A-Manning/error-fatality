//! Tests for `#[derive(Fatality)]`

use proc_macro2::TokenStream;
use quote::quote;

fn run_test(input: TokenStream, expected: TokenStream) {
    let output = crate::derive_fatality2(input);
    let output = output.to_string();
    assert_eq!(output, expected.to_string());
}

#[test]
fn transparent_fatal_explitit() {
    run_test(
        quote! {
            enum Q {
                #[error(transparent)]
                #[fatal(true)]
                V(I),
            }
        },
        quote! {
            #[automatically_derived]
            impl crate::Fatality for Q {
                fn is_fatal(&self) -> bool {
                    match self {
                        Self::V(..) => true,
                    }
                }
            }
        },
    );
}

#[test]
fn transparent_fatal_fwd() {
    run_test(
        quote! {
            enum Q {
                #[error(transparent)]
                #[fatal(forward)]
                V(I),
            }
        },
        quote! {
            #[automatically_derived]
            impl crate::Fatality for Q {
                fn is_fatal(&self) -> bool {
                    match self {
                        Self::V(arg_0, ..) => <_ as crate::Fatality>::is_fatal(arg_0),
                    }
                }
            }
        },
    );
}

#[test]
fn transparent_fatal_true() {
    run_test(
        quote! {
            enum Q {
                #[error(transparent)]
                #[fatal(true)]
                V(I),
            }
        },
        quote! {
            #[automatically_derived]
            impl crate::Fatality for Q {
                fn is_fatal(&self) -> bool {
                    match self {
                        Self::V(..) => true,
                    }
                }
            }
        },
    );
}

#[test]
fn source_fatal() {
    run_test(
        quote! {
            enum Q {
                #[error("DDDDDDDDDDDD")]
                #[fatal(forward)]
                V(first, #[source] I),
            }
        },
        quote! {
            #[automatically_derived]
            impl crate::Fatality for Q {
                fn is_fatal(&self) -> bool {
                    match self {
                        Self::V(_, arg_1, ..) => <_ as crate::Fatality>::is_fatal(arg_1),
                    }
                }
            }
        },
    );
}

#[test]
fn full() {
    run_test(
        quote! {
            enum Kaboom {
                #[error(transparent)]
                #[fatal(forward)]
                // only one arg, that's ok, the first will be used
                A(X),

                #[error("Bar")]
                #[fatal(forward)]
                B(#[source] Y),

                #[error("zzzZZzZ")]
                #[fatal(forward)]
                C {#[source] z: Z },

                #[error("What?")]
                #[fatal(false)]
                What,


                #[error(transparent)]
                #[fatal(true)]
                O(P),
            }
        },
        quote! {
            #[automatically_derived]
            impl crate::Fatality for Kaboom {
                fn is_fatal(&self) -> bool {
                    match self {
                        Self::A(arg_0, ..) => <_ as crate::Fatality>::is_fatal(arg_0),
                        Self::B(arg_0, ..) => <_ as crate::Fatality>::is_fatal(arg_0),
                        Self::C{z, ..} => <_ as crate::Fatality>::is_fatal(z),
                        Self::What => false,
                        Self::O(..) => true,
                    }
                }
            }
        },
    );
}

#[test]
fn strukt_01_forward() {
    run_test(
        quote! {
            #[fatal(forward)]
            pub struct X {
                #[source]
                inner: InnerError,
            }
        },
        quote! {
            #[automatically_derived]
            impl crate :: Fatality for X {
                fn is_fatal (& self) -> bool {
                    crate::Fatality::is_fatal(&self.inner)
                }
            }
        },
    );
}

#[test]
fn strukt_02_explicit_fatal() {
    run_test(
        quote! {
            #[error("Mission abort. Maybe?")]
            #[fatal(true)]
            pub struct X {
                #[source]
                inner: InnerError,
            }
        },
        quote! {
            #[automatically_derived]
            impl crate :: Fatality for X {
                fn is_fatal (& self) -> bool {
                    true
                }
            }
        },
    );
}

#[test]
fn strukt_03_explicit_jfyi() {
    run_test(
        quote! {
            #[error("Mission abort. Maybe?")]
            #[fatal(false)]
            pub struct X {
                #[source]
                inner: InnerError,
            }
        },
        quote! {
            #[automatically_derived]
            impl crate :: Fatality for X {
                fn is_fatal (& self) -> bool {
                    false
                }
            }
        },
    );
}
