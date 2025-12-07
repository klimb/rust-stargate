use proc_macro::TokenStream;
use quote::quote;

pub fn to_object(stream: TokenStream) -> TokenStream {
    let stream = proc_macro2::TokenStream::from(stream);

    let new = quote!(
        pub fn to_obj(args: impl sgcore::CommonArgs) -> i32 {
            #stream
            sgcore::disable_rust_signal_handlers().expect("Disabling rust signal handlers failed");
            let result = to_obj(args);
            match result {
                Success(()) => sgcore::error::get_exit_code(),
                Error(e) => {
                    let s = format!("{e}");
                    if s != "" {
                        sgcore::show_error!("{s}");
                    }
                    if e.usage() {
                        eprintln!("Try '{} --help' for more information.", sgcore::execution_phrase());
                    }
                    e.code()
                }
            }
        }
    );

    TokenStream::from(new)
}
