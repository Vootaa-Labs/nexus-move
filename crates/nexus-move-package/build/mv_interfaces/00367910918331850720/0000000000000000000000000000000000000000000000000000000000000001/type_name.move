module 0x1::type_name {

    use 0x1::ascii;
    use 0x1::type_name;

    struct TypeName has copy, drop, store {
        name: ascii::String,
    }

    // NOTE: Functions are 'native' for simplicity. They may or may not be native in actuality.
    #[native_interface]
    native public fun borrow_string(a0: &type_name::TypeName): &ascii::String;
    #[native_interface]
    native public fun get<T0>(): type_name::TypeName;
    #[native_interface]
    native public fun into_string(a0: type_name::TypeName): ascii::String;

}
