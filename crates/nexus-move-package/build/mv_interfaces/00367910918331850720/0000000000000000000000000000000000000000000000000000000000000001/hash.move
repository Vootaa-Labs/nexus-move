module 0x1::hash {

    // NOTE: Functions are 'native' for simplicity. They may or may not be native in actuality.
    #[native_interface]
    native public fun sha2_256(a0: vector<u8>): vector<u8>;
    #[native_interface]
    native public fun sha3_256(a0: vector<u8>): vector<u8>;

}
