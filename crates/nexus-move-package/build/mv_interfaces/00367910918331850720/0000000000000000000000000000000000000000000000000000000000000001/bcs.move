module 0x1::bcs {

    // NOTE: Functions are 'native' for simplicity. They may or may not be native in actuality.
    #[native_interface]
    native public fun to_bytes<T0>(a0: &T0): vector<u8>;

}
