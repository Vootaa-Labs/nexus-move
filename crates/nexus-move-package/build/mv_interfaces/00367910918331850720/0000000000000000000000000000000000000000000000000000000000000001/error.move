module 0x1::error {

    // NOTE: Functions are 'native' for simplicity. They may or may not be native in actuality.
    #[native_interface]
    native public fun aborted(a0: u64): u64;
    #[native_interface]
    native public fun already_exists(a0: u64): u64;
    #[native_interface]
    native public fun canonical(a0: u64, a1: u64): u64;
    #[native_interface]
    native public fun internal(a0: u64): u64;
    #[native_interface]
    native public fun invalid_argument(a0: u64): u64;
    #[native_interface]
    native public fun invalid_state(a0: u64): u64;
    #[native_interface]
    native public fun not_found(a0: u64): u64;
    #[native_interface]
    native public fun not_implemented(a0: u64): u64;
    #[native_interface]
    native public fun out_of_range(a0: u64): u64;
    #[native_interface]
    native public fun permission_denied(a0: u64): u64;
    #[native_interface]
    native public fun resource_exhausted(a0: u64): u64;
    #[native_interface]
    native public fun unauthenticated(a0: u64): u64;
    #[native_interface]
    native public fun unavailable(a0: u64): u64;

}
