//! Tests for `#[derive(Fatality)]`

use proc_macro2::TokenStream;
use quote::quote;

fn run_test(input: TokenStream, expected: TokenStream) {
    let output = crate::derive_fatality2(input);
    let output = output.to_string();
    println!(
        r##">>>>>>>>>>>>>>>>>>>
{}
>>>>>>>>>>>>>>>>>>>"##,
        output.as_str()
    );
    assert_eq!(output, expected.to_string(),);
}

#[test]
fn transparent_fatal_implicit() {
    run_test(
        quote! {
            enum Q {
                #[fatal]
                #[error(transparent)]
                V(I),
            }
        },
        quote! {
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
                #[fatal(forward)]
                #[error(transparent)]
                V(I),
            }
        },
        quote! {
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
                #[fatal(true)]
                #[error(transparent)]
                V(I),
            }
        },
        quote! {
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
                #[fatal(forward)]
                #[error("DDDDDDDDDDDD")]
                V(first, #[source] I),
            }
        },
        quote! {
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
                #[fatal(forward)]
                #[error(transparent)]
                // only one arg, that's ok, the first will be used
                A(X),

                #[fatal(forward)]
                #[error("Bar")]
                B(#[source] Y),

                #[fatal(forward)]
                #[error("zzzZZzZ")]
                C {#[source] z: Z },

                #[error("What?")]
                What,


                #[fatal]
                #[error(transparent)]
                O(P),
            }
        },
        quote! {
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
            pub struct X {
                #[source]
                inner: InnerError,
            }
        },
        quote! {
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
            #[fatal(true)]
            #[error("Mission abort. Maybe?")]
            pub struct X {
                #[source]
                inner: InnerError,
            }
        },
        quote! {
            impl crate :: Fatality for X {
                fn is_fatal (& self) -> bool {
                    true
                }
            }
        },
    );
}

#[test]
fn strukt_03_implicit_fatal() {
    run_test(
        quote! {
            #[fatal]
            #[error("Mission abort. Maybe?")]
            pub struct X {
                #[source]
                inner: InnerError,
            }
        },
        quote! {
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
            #[fatal(false)]
            #[error("Mission abort. Maybe?")]
            pub struct X {
                #[source]
                inner: InnerError,
            }
        },
        quote! {
            impl crate :: Fatality for X {
                fn is_fatal (& self) -> bool {
                    false
                }
            }
        },
    );
}
