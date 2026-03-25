module 0x1::event {

    use 0x1::event;
    use 0x1::guid;

    struct EventHandle<phantom T0: drop+ store> has store {
        counter: u64,
        guid: event::GUIDWrapper,
    }
    struct EventHandleGenerator has key {
        counter: u64,
        addr: address,
    }
    struct GUIDWrapper has drop, store {
        len_bytes: u8,
        guid: guid::GUID,
    }

    // NOTE: Functions are 'native' for simplicity. They may or may not be native in actuality.
    #[native_interface]
    native public fun guid<T0: drop+ store>(a0: &event::EventHandle<T0>): &guid::GUID;
    #[native_interface]
    native public fun destroy_handle<T0: drop+ store>(a0: event::EventHandle<T0>);
    #[native_interface]
    native public fun emit_event<T0: drop+ store>(a0: &mut event::EventHandle<T0>, a1: T0);
    #[native_interface]
    native public fun new_event_handle<T0: drop+ store>(a0: &signer): event::EventHandle<T0>;

}
