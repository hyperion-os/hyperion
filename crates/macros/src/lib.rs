use std::{iter, process::Command};

use chrono::{Datelike, Utc};
use proc_macro2::Span;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Expr, LitInt, LitStr, Result, Token,
};

//

#[proc_macro]
pub fn array(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    struct Input {
        f: Expr,
        _semi: Token![;],
        len: LitInt,
    }

    impl Parse for Input {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(Input {
                f: input.parse()?,
                _semi: input.parse()?,
                len: input.parse()?,
            })
        }
    }

    let Input { f, len, .. } = parse_macro_input!(input as Input);

    let array = iter::repeat(f).take(len.base10_parse().unwrap());

    (quote! {
        [#(#array),*]
    })
    .into()
}

#[proc_macro]
pub fn gen_int_handlers(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ext: LitStr = parse_macro_input!(input as LitStr);

    let ints = (32u8..=255).map(|i| {
        let ident = syn::Ident::new(&format!("int_handler_{i}"), Span::call_site());
        quote! {
            pub extern #ext fn #ident(_: InterruptStackFrame) {
                interrupt_handler(#i);
            }
        }
    });

    (quote! {
        #(#ints)*
    })
    .into()
}

#[proc_macro]
pub fn get_int_handlers(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    if let Some(err) = expect_empty(input) {
        return err;
    }

    let ints = (32u8..=255).map(|i| {
        let ident = syn::Ident::new(&format!("int_handler_{i}"), Span::call_site());
        quote! {
            (#i, #ident as _),
        }
    });

    (quote! {
        [#(#ints)*]
    })
    .into()
}

#[proc_macro]
pub fn rtc_year(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    if let Some(err) = expect_empty(input) {
        return err;
    }

    let year: u32 = Utc::now().date_naive().year() as u32;

    (quote! {
        #year
    })
    .into()
}

#[proc_macro]
pub fn build_time(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    if let Some(err) = expect_empty(input) {
        return err;
    }

    let time = Utc::now()
        .naive_local()
        .format("%d/%m/%Y %H:%M:%S")
        .to_string();

    (quote! {
        #time
    })
    .into()
}

#[proc_macro]
pub fn build_rev(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    if let Some(err) = expect_empty(input) {
        return err;
    }

    let res = Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .unwrap()
        .stdout;
    let rev = std::str::from_utf8(&res).unwrap().trim();

    (quote! {
        #rev
    })
    .into()
}

fn expect_empty(input: proc_macro::TokenStream) -> Option<proc_macro::TokenStream> {
    if !input.is_empty() {
        return Some(
            (quote! {
                compile_error!("expected zero tokens")
            })
            .into(),
        );
    }
    None
}
