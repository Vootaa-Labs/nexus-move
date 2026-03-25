module 0x1::bit_vector {

    use 0x1::bit_vector;

    struct BitVector has copy, drop, store {
        length: u64,
        bit_field: vector<bool>,
    }

    // NOTE: Functions are 'native' for simplicity. They may or may not be native in actuality.
    #[native_interface]
    native public fun length(a0: &bit_vector::BitVector): u64;
    #[native_interface]
    native public fun is_index_set(a0: &bit_vector::BitVector, a1: u64): bool;
    #[native_interface]
    native public fun longest_set_sequence_starting_at(a0: &bit_vector::BitVector, a1: u64): u64;
    #[native_interface]
    native public fun new(a0: u64): bit_vector::BitVector;
    #[native_interface]
    native public fun set(a0: &mut bit_vector::BitVector, a1: u64);
    #[native_interface]
    native public fun shift_left(a0: &mut bit_vector::BitVector, a1: u64);
    #[native_interface]
    native public fun unset(a0: &mut bit_vector::BitVector, a1: u64);

}
