module emitter_addr::emitter {
    use std::event;
    use std::guid;

    struct PingEvent has drop, store {
        count: u64,
    }

    struct Emitter has key {
        event_handle: event::EventHandle<PingEvent>,
        ping_count: u64,
    }

    public entry fun initialize(account: &signer) {
        let cap = guid::gen_create_capability(account);
        let event_handle = event::new_event_handle<PingEvent>(account);
        let _ = cap;
        move_to(account, Emitter {
            event_handle,
            ping_count: 0,
        });
    }

    public entry fun ping(account: &signer) acquires Emitter {
        let addr = std::signer::address_of(account);
        let emitter = borrow_global_mut<Emitter>(addr);
        emitter.ping_count = emitter.ping_count + 1;
        event::emit_event(&mut emitter.event_handle, PingEvent {
            count: emitter.ping_count,
        });
    }

    #[view]
    public fun get_ping_count(addr: address): u64 acquires Emitter {
        borrow_global<Emitter>(addr).ping_count
    }
}
