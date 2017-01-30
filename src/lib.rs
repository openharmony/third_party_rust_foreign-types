//! A framework for Rust wrappers over C APIs.
//!
//! Ownership is as important in C as it is in Rust, but the semantics are often implicit. In
//! particular, pointer-to-value is commonly used to pass C values both when transferring ownership
//! or a borrow.
//!
//! This crate provides a framework to define a Rust wrapper over these kinds of raw C APIs in a way
//! that allows ownership semantics to be expressed in an ergonomic manner. The framework takes a
//! dual-type approach similar to APIs in the standard library such as `PathBuf`/`Path` or `String`/
//! `&str`. One type represents an owned value and references to the other represent borrowed
//! values.
//!
//! # Examples
//!
//! ```
//! use foreign_types::{ForeignType, ForeignTypeRef, Opaque};
//! use std::ops::{Deref, DerefMut};
//!
//! mod foo_sys {
//!     pub enum FOO {}
//!
//!     extern {
//!         pub fn FOO_free(foo: *mut FOO);
//!     }
//! }
//!
//! // The borrowed type is a newtype wrapper around an `Opaque` value.
//! //
//! // `FooRef` values never exist; we instead create references to `FooRef`s from raw C pointers.
//! pub struct FooRef(Opaque);
//!
//! impl ForeignTypeRef for FooRef {
//!     type CType = foo_sys::FOO;
//! }
//!
//! // The owned type is simply a newtype wrapper around the raw C type.
//! //
//! // It dereferences to `FooRef`, so methods that do not require ownership should be defined
//! // there.
//! pub struct Foo(*mut foo_sys::FOO);
//!
//! impl Drop for Foo {
//!     fn drop(&mut self) {
//!         unsafe { foo_sys::FOO_free(self.0) }
//!     }
//! }
//!
//! impl ForeignType for Foo {
//!     type CType = foo_sys::FOO;
//!     type Ref = FooRef;
//!
//!     unsafe fn from_ptr(ptr: *mut foo_sys::FOO) -> Foo {
//!         Foo(ptr)
//!     }
//! }
//!
//! impl Deref for Foo {
//!     type Target = FooRef;
//!
//!     fn deref(&self) -> &FooRef {
//!         unsafe { FooRef::from_ptr(self.0) }
//!     }
//! }
//!
//! impl DerefMut for Foo {
//!     fn deref_mut(&mut self) -> &mut FooRef {
//!         unsafe { FooRef::from_ptr_mut(self.0) }
//!     }
//! }
//! ```
//!
//! The `foreign_type!` macro can generate this boilerplate for you:
//!
//! ```
//! #[macro_use]
//! extern crate foreign_types;
//!
//! mod foo_sys {
//!     pub enum FOO {}
//!
//!     extern {
//!         pub fn FOO_free(foo: *mut FOO);
//!     }
//! }
//!
//! foreign_type! {
//!     /// A Foo.
//!     owned: Foo;
//!     /// A borrowed Foo.
//!     borrowed: FooRef;
//!     ctype: foo_sys::FOO;
//!     drop: foo_sys::FOO_free;
//! }
//!
//! # fn main() {}
//! ```
//!
//! Say we then have a separate type in our C API that contains a `FOO`:
//!
//! ```
//! mod foo_sys {
//!     pub enum FOO {}
//!     pub enum BAR {}
//!
//!     extern {
//!         pub fn FOO_free(foo: *mut FOO);
//!         pub fn BAR_free(bar: *mut BAR);
//!         pub fn BAR_get_foo(bar: *mut BAR) -> *mut FOO;
//!     }
//! }
//! ```
//!
//! The documentation for the C library states that `BAR_get_foo` returns a reference into the `BAR`
//! passed to it, which translates into a reference in Rust. It also says that we're allowed to
//! modify the `FOO`, so we'll define a pair of accessor methods, one immutable and one mutable:
//!
//! ```
//! #[macro_use]
//! extern crate foreign_types;
//!
//! use foreign_types::ForeignTypeRef;
//!
//! mod foo_sys {
//!     pub enum FOO {}
//!     pub enum BAR {}
//!
//!     extern {
//!         pub fn FOO_free(foo: *mut FOO);
//!         pub fn BAR_free(bar: *mut BAR);
//!         pub fn BAR_get_foo(bar: *mut BAR) -> *mut FOO;
//!     }
//! }
//!
//! foreign_type! {
//!     /// A Foo.
//!     owned: Foo;
//!     /// A borrowed Foo.
//!     borrowed: FooRef;
//!     ctype: foo_sys::FOO;
//!     drop: foo_sys::FOO_free;
//! }
//!
//! foreign_type! {
//!     /// A Bar.
//!     owned: Bar;
//!     /// A borrowed Bar.
//!     borrowed: BarRef;
//!     ctype: foo_sys::BAR;
//!     drop: foo_sys::BAR_free;
//! }
//!
//! impl BarRef {
//!     fn foo(&self) -> &FooRef {
//!         unsafe { FooRef::from_ptr(foo_sys::BAR_get_foo(self.as_ptr())) }
//!     }
//!
//!     fn foo_mut(&mut self) -> &mut FooRef {
//!         unsafe { FooRef::from_ptr_mut(foo_sys::BAR_get_foo(self.as_ptr())) }
//!     }
//! }
//!
//! # fn main() {}
//! ```
use std::cell::UnsafeCell;

/// An opaque type used to define `ForeignTypeRef` types.
///
/// A type designed to implement `ForeignTypeRef` should simply be a newtype wrapper around this
/// type. It has an `UnsafeCell` internally to inform the compiler about aliasability.
pub struct Opaque(UnsafeCell<()>);

/// A type implemented by wrappers over foreign types.
pub trait ForeignType: Sized {
    /// The raw C type.
    type CType;

    /// The type representing a reference to this type.
    type Ref: ForeignTypeRef<CType = Self::CType>;

    /// Constructs an instance of this type from its raw type.
    unsafe fn from_ptr(ptr: *mut Self::CType) -> Self;
}

/// A trait implemented by types which reference borrowed foreign types.
pub trait ForeignTypeRef: Sized {
    /// The raw C type.
    type CType;

    /// Constructs a shared instance of this type from its raw type.
    unsafe fn from_ptr<'a>(ptr: *mut Self::CType) -> &'a Self {
        &*(ptr as *mut _)
    }

    /// Constructs a mutable reference of this type from its raw type.
    unsafe fn from_ptr_mut<'a>(ptr: *mut Self::CType) -> &'a mut Self {
        &mut *(ptr as *mut _)
    }

    /// Returns a raw pointer to the wrapped value.
    fn as_ptr(&self) -> *mut Self::CType {
        self as *const _ as *mut _
    }
}

/// A macro to easily define wrappers for foreign types.
///
/// # Examples
///
/// ```
/// #[macro_use]
/// extern crate foreign_types;
///
/// # mod openssl_sys { pub type SSL = (); pub unsafe fn SSL_free(_: *mut SSL) {} }
/// foreign_type! {
///     /// Documentation for the owned type.
///     owned: Ssl;
///     /// Documentation for the borrowed type.
///     borrowed: SslRef;
///     ctype: openssl_sys::SSL;
///     drop: openssl_sys::SSL_free;
/// }
///
/// # fn main() {}
/// ```
#[macro_export]
macro_rules! foreign_type {
    (
        $(#[$owned_attr:meta])*
        owned: $owned:ident;
        $(#[$borrowed_attr:meta])*
        borrowed: $borrowed:ident;
        ctype: $ctype:ty;
        drop: $drop:expr;
    ) => {
        $(#[$owned_attr])*
        pub struct $owned(*mut $ctype);

        impl $crate::ForeignType for $owned {
            type CType = $ctype;
            type Ref = $borrowed;

            unsafe fn from_ptr(ptr: *mut $ctype) -> $owned {
                $owned(ptr)
            }
        }

        impl Drop for $owned {
            fn drop(&mut self) {
                unsafe { $drop(self.0) }
            }
        }

        impl ::std::ops::Deref for $owned {
            type Target = $borrowed;

            fn deref(&self) -> &$borrowed {
                unsafe { $crate::ForeignTypeRef::from_ptr(self.0) }
            }
        }

        impl ::std::ops::DerefMut for $owned {
            fn deref_mut(&mut self) -> &mut $borrowed {
                unsafe { $crate::ForeignTypeRef::from_ptr_mut(self.0) }
            }
        }

        $(#[$borrowed_attr])*
        pub struct $borrowed($crate::Opaque);

        impl $crate::ForeignTypeRef for $borrowed {
            type CType = $ctype;
        }
    }
}