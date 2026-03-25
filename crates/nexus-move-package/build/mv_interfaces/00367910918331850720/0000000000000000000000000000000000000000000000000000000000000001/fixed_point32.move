module 0x1::fixed_point32 {

    use 0x1::fixed_point32;

    struct FixedPoint32 has copy, drop, store {
        value: u64,
    }

    // NOTE: Functions are 'native' for simplicity. They may or may not be native in actuality.
    #[native_interface]
    native public fun ceil(a0: fixed_point32::FixedPoint32): u64;
    #[native_interface]
    native public fun create_from_rational(a0: u64, a1: u64): fixed_point32::FixedPoint32;
    #[native_interface]
    native public fun create_from_raw_value(a0: u64): fixed_point32::FixedPoint32;
    #[native_interface]
    native public fun create_from_u64(a0: u64): fixed_point32::FixedPoint32;
    #[native_interface]
    native public fun divide_u64(a0: u64, a1: fixed_point32::FixedPoint32): u64;
    #[native_interface]
    native public fun floor(a0: fixed_point32::FixedPoint32): u64;
    #[native_interface]
    native public fun get_raw_value(a0: fixed_point32::FixedPoint32): u64;
    #[native_interface]
    native public fun is_zero(a0: fixed_point32::FixedPoint32): bool;
    #[native_interface]
    native public fun max(a0: fixed_point32::FixedPoint32, a1: fixed_point32::FixedPoint32): fixed_point32::FixedPoint32;
    #[native_interface]
    native public fun min(a0: fixed_point32::FixedPoint32, a1: fixed_point32::FixedPoint32): fixed_point32::FixedPoint32;
    #[native_interface]
    native public fun multiply_u64(a0: u64, a1: fixed_point32::FixedPoint32): u64;
    #[native_interface]
    native public fun round(a0: fixed_point32::FixedPoint32): u64;

}
