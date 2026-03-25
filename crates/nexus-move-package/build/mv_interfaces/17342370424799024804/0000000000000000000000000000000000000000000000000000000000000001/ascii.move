module 0x1::ascii {

    use 0x1::ascii;
    use 0x1::option;

    struct Char has copy, drop, store {
        byte: u8,
    }
    struct String has copy, drop, store {
        bytes: vector<u8>,
    }

    // NOTE: Functions are 'native' for simplicity. They may or may not be native in actuality.
    #[native_interface]
    native public fun length(a0: &ascii::String): u64;
    #[native_interface]
    native public fun all_characters_printable(a0: &ascii::String): bool;
    #[native_interface]
    native public fun string(a0: vector<u8>): ascii::String;
    #[native_interface]
    native public fun as_bytes(a0: &ascii::String): &vector<u8>;
    #[native_interface]
    native public fun byte(a0: ascii::Char): u8;
    #[native_interface]
    native public fun char(a0: u8): ascii::Char;
    #[native_interface]
    native public fun into_bytes(a0: ascii::String): vector<u8>;
    #[native_interface]
    native public fun is_printable_char(a0: u8): bool;
    #[native_interface]
    native public fun is_valid_char(a0: u8): bool;
    #[native_interface]
    native public fun pop_char(a0: &mut ascii::String): ascii::Char;
    #[native_interface]
    native public fun push_char(a0: &mut ascii::String, a1: ascii::Char);
    #[native_interface]
    native public fun try_string(a0: vector<u8>): option::Option<ascii::String>;

}
