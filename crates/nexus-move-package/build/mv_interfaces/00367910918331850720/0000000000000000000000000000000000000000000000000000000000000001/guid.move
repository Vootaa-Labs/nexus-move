module 0x1::guid {

    use 0x1::guid;

    struct CreateCapability has drop, store, key {
        addr: address,
    }
    struct GUID has drop, store {
        id: guid::ID,
    }
    struct Generator has key {
        counter: u64,
    }
    struct ID has copy, drop, store {
        creation_num: u64,
        addr: address,
    }

    // NOTE: Functions are 'native' for simplicity. They may or may not be native in actuality.
    #[native_interface]
    native public fun create(a0: &signer): guid::GUID;
    #[native_interface]
    native public fun create_id(a0: address, a1: u64): guid::ID;
    #[native_interface]
    native public fun creation_num(a0: &guid::GUID): u64;
    #[native_interface]
    native public fun create_with_capability(a0: address, a1: &guid::CreateCapability): guid::GUID;
    #[native_interface]
    native public fun creator_address(a0: &guid::GUID): address;
    #[native_interface]
    native public fun eq_id(a0: &guid::GUID, a1: &guid::ID): bool;
    #[native_interface]
    native public fun id(a0: &guid::GUID): guid::ID;
    #[native_interface]
    native public fun gen_create_capability(a0: &signer): guid::CreateCapability;
    #[native_interface]
    native public fun get_next_creation_num(a0: address): u64;
    #[native_interface]
    native public fun id_creation_num(a0: &guid::ID): u64;
    #[native_interface]
    native public fun id_creator_address(a0: &guid::ID): address;
    #[native_interface]
    native public fun publish_generator(a0: &signer);

}
