//! # Easy-FFI: A helper macro for FFI helper macros
//!
//! This crate attempts to make the process of writing an unwind-safe C api more ergonomic.
//!
//! # What this crate does
//!
//! * Prevents unwinding across the FFI boundary
//! * Allows the use of the usual Rust error handling idioms
//!
//! # What this crate does *not* do
//!
//! * Prevent you from dereferencing invalid pointers
//! * Prevent memory leaks
//! * Any kind of validation of arguments or returns from your FFI functions
//!
//! # Example
//!
//! ## Without `easy_ffi`:
//!
//! ```
//! fn thing_that_could_fail_or_panic() -> Result<i32, &'static str> {
//!     // Do stuff...
//! #   Ok(5)
//! }
//!
//! #[no_mangle]
//! pub extern "C" fn my_ffi_function(i: i32) -> i32 {
//!     // Unwinding over the FFI boundary is UB, so we need to catch panics
//!     let panic_result: Result<i32, _> = ::std::panic::catch_unwind(move || {
//!         let result_one = thing_that_could_fail_or_panic();
//!
//!         // We need to match on this result to handle the possible Result::Err
//!         // and convert it to a senssible ffi representation.
//!         match result_one {
//!             Ok(actual) => return actual,
//!             Err(e) => {
//!                 println!("Oops! {:?}", e);
//!                 return -1;
//!             }
//!         }
//!     });
//!
//!     // Then, we need to match on the catch_unwind result again like we did for the Result::Err
//!     match panic_result {
//!         Ok(actual) => return actual,
//!         Err(_e) => {
//!             println!("unexpected panic!");
//!             return -1;
//!         }
//!     }
//! }
//! ```
//!
//! Using only rust std, anything that could potentially panic needs to be
//! wrapped with `catch_unwind` to prevent unwinding into C. Also, since FFI functions
//! won't be returning Rust's `Result<T, E>`, you're prevented from using `try!` or `?`
//! for error-handling ergonomics.
//!
//! ## With `easy_ffi`:
//!
//! ```
//! # #[macro_use] extern crate easy_ffi;
//!
//! fn thing_that_could_fail_or_panic() -> Result<i32, &'static str> {
//!     // Do stuff...
//! #   Ok(5)
//! }
//!
//! // This defines a new macro that will be used to wrap a more "rusty"
//! // version of our ffi function.
//! easy_ffi!(my_ffi_fn =>
//!     // Now we define a handler for each of the error cases: A `Result::Err` and
//!     // a caught panic. `Result::Err` comes first:
//!     |err| {
//!         println!("{}", err);
//!         // The handler still needs to return the actual type that the C api expects,
//!         // so we're going to do so here:
//!         -1
//!     }
//!     // Next, the panic. This will have the type `Box<dyn Any + Send + 'static>`. See
//!     // `::std::panic::catch_unwind` for more details.
//!     |panic_val| {
//!         match panic_val.downcast_ref::<&'static str>() {
//!             Some(s) => println!("panic: {}", s),
//!             None => println!("unknown panic!"),
//!         }
//!         // As with the error handler, the panic handler also needs to return
//!         // the real ffi return type.
//!         -1
//!     }
//! );
//!
//! // Using the new macro that `easy_ffi!` created for us, we can write our
//! // function just like any Rust function that returns a `Result`. This will
//! // automatically be wrapped in a `catch_unwind`, and error handling will be
//! // left to the "handler" that was defined in the call to `easy_ffi`.
//! my_ffi_fn!(
//!     /// You can put doc comments here!
//!     ///
//!     /// This should generate a function with the signature `fn(i32) -> i32`,
//!     /// with all of the necessary `pub`, `#[no_mangle]`, `extern "C"`, etc.
//!     fn foo(i: i32) -> Result<i32, &'static str> {
//!         thing_that_could_fail_or_panic()
//!     }
//! );
//! # fn main() {}
//! ```

#[macro_export]
macro_rules! easy_ffi {
    ($name:ident => |$err:ident| $err_body:tt |$panic:ident| $panic_body:tt) => (
        easy_ffi!(@actual ($) $name $err $err_body $panic $panic_body);
    );
    (@actual ($dol:tt) $name:ident $err:ident $err_body:tt $panic:ident $panic_body:tt) => {
        macro_rules! $name {
            (
                $dol (#[$dol attr:meta])*
                fn $dol fn_name:ident (
                    $dol ($dol arg:ident : $dol arg_ty:ty),* $dol (,)*
                ) -> Result<$dol ok_ty:ty, $dol err_ty:ty>
                $dol body:tt
            ) => (
                #[no_mangle]
                $dol (#[$attr])*
                pub extern "C" fn $fn_name($dol ($arg : $arg_ty),*) -> $ok_ty {
                    let safe_res:
                        ::std::result::Result<$ok_ty, ::std::result::Result<$err_ty, Box<dyn (::std::any::Any) + Send + 'static>>> =
                        ::std::panic::catch_unwind(move || $body)
                            .map_err(|e| ::std::result::Result::Err(e))
                            .and_then(|ok| ok.map_err(|e| ::std::result::Result::Ok(e)));
                    match safe_res {
                        Ok(x) => return x,
                        Err(Ok($err)) => $err_body,
                        Err(Err($panic)) => $panic_body,
                    }
                }
            );
        }
    };
}

#[cfg(test)]
mod tests {
    #![allow(private_no_mangle_fns)]

    easy_ffi!(my_ffi_fn =>
        |err| {
            println!("{}", err);
            -1
        }
        |panic_val| {
            match panic_val.downcast_ref::<&'static str>() {
                Some(s) => println!("panic: {}", s),
                None => println!("unknown panic!"),
            };
            -1
        }
    );

    my_ffi_fn! (
        /// Foo: do stuff
        fn foo(i: i32) -> Result<i32, &'static str> {
            match i {
                5 => panic!("I'm afraid of 5's!"),
                i if i <= 0 => Err("already <= 0, can't go lower"),
                i => Ok(i-1),
            }
        }
    );

    #[test]
    fn it_works() {
        assert_eq!(-1, foo(5));
        assert_eq!(-1, foo(0));
        assert_eq!(0, foo(1));
        assert_eq!(1, foo(2));
    }
}
