module 0x1::string {

    use 0x1::option;
    use 0x1::string;

    struct String has copy, drop, store {
        bytes: vector<u8>,
    }

    // NOTE: Functions are 'native' for simplicity. They may or may not be native in actuality.
    #[native_interface]
    native public fun index_of(a0: &string::String, a1: &string::String): u64;
    #[native_interface]
    native public fun append(a0: &mut string::String, a1: string::String);
    #[native_interface]
    native public fun is_empty(a0: &string::String): bool;
    #[native_interface]
    native public fun length(a0: &string::String): u64;
    #[native_interface]
    native public fun bytes(a0: &string::String): &vector<u8>;
    #[native_interface]
    native public fun append_utf8(a0: &mut string::String, a1: vector<u8>);
    #[native_interface]
    native public fun insert(a0: &mut string::String, a1: u64, a2: string::String);
    #[native_interface]
    native public fun sub_string(a0: &string::String, a1: u64, a2: u64): string::String;
    #[native_interface]
    native public fun try_utf8(a0: vector<u8>): option::Option<string::String>;
    #[native_interface]
    native public fun utf8(a0: vector<u8>): string::String;

}
