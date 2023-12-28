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
            pub extern #ext fn #ident(frame: InterruptStackFrame) {
                interrupt_handler(#i, frame.instruction_pointer.as_u64() as usize);
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

/* #[proc_macro_attribute]
pub fn trace(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    // struct Input {
    //     attrs: Vec<Attribute>,
    //     vis: Visibility,
    //     sig: Signature,
    //     _block: Brace,
    //     block_contents: TokenStream,
    // }

    // impl Parse for Input {
    //     fn parse(input: ParseStream) -> Result<Self> {
    //         let block_contents;
    //         Ok(Self {
    //             attrs: Attribute::parse_outer(input)?,
    //             vis: input.parse()?,
    //             sig: input.parse()?,
    //             _block: braced!(block_contents in input),
    //             block_contents: block_contents.parse()?,
    //         })
    //     }
    // }

    // let Input {
    //     attrs,
    //     vis,
    //     sig,
    //     block_contents,
    //     ..
    // } = parse_macro_input!(input as Input);

    // (quote! {
    //     #(#attrs)*
    //     #vis #sig {
    //         #block_contents
    //     }
    // })
    // .into()

    let mut fn_item = parse_macro_input!(input as ItemFn);

    let real_vis = fn_item.vis;
    fn_item.vis = Visibility::Inherited;

    let real_sig = fn_item.sig.clone();
    fn_item.sig.ident = Ident::new(&format!("_real_{}", fn_item.sig.ident), Span::call_site());

    let real_fn = &fn_item.sig.ident;
    let call_id = real_sig.ident.to_string();
    let call_id = call_id.trim_start_matches('_');

    let args: Vec<_> = fn_item
        .sig
        .inputs
        .iter()
        .map(|arg| match arg {
            syn::FnArg::Receiver(_) => panic!("`self` not supported"),
            syn::FnArg::Typed(PatType { pat, .. }) => (**pat).clone(),
        })
        .collect();

    let call_dbg = args
        .iter()
        .map(|id| id.to_token_stream())
        .fold(String::new(), |mut acc, v| {
            use std::fmt::Write;
            write!(acc, ", {v}: {{:?}}").unwrap();
            acc
        });
    let call_dbg = format!("syscall::{call_id}({})", call_dbg.trim_start_matches(", "));

    if !attr.is_empty() {
        // split into `name(args)` and ` = result`
        // calls like `exit` won't return and a split is necessary
        (quote! {
            #real_vis #real_sig {
                #fn_item

                if LOG_SYSCALLS {
                    debug!(#call_dbg, #(&#args,)*);
                }

                let result = #real_fn(#(#args,)*);

                if LOG_SYSCALLS {
                    debug!(" = {result:?}");
                }

                result
            }

        })
        .into()
    } else {
        // or don't split
        let call_dbg = format!("{call_dbg} = {{result:?}}");
        (quote! {
            #real_vis #real_sig {
                #fn_item

                let result = #real_fn(#(#args,)*);

                if LOG_SYSCALLS {
                    debug!(#call_dbg, #(&#args,)*);
                }

                result
            }

        })
        .into()
    }
} */
