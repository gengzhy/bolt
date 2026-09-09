use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use lt_task::{App, LtEvent};

#[test]
fn test_connect_disconnect_twice() {
    lt_crypto::ensure_provider();

    let temp_root = std::env::temp_dir().join(format!("lt_disc_{}", std::process::id()));
    let dir_a = temp_root.join("client_a");
    let dir_b = temp_root.join("client_b");
    let _ = std::fs::remove_dir_all(&temp_root);
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();

    let app_a = App::init(Some(dir_a.clone())).expect("init A");
    let app_b = App::init(Some(dir_b.clone())).expect("init B");

    let uuid_a = app_a.device_uuid();
    let uuid_b = app_b.device_uuid();
    let fp_a = app_a.local_fingerprint();
    let fp_b = app_b.local_fingerprint();
    let port_b = app_b.engine_port();

    app_a.trust().add(&uuid_b, &fp_b, "ClientB").unwrap();
    app_b.trust().add(&uuid_a, &fp_a, "ClientA").unwrap();

    let events_a = Arc::new(Mutex::new(Vec::new()));
    let ev_a = events_a.clone();
    app_a.set_event_sink(move |ev| {
        ev_a.lock().unwrap().push(ev);
    });

    let events_b = Arc::new(Mutex::new(Vec::new()));
    let ev_b = events_b.clone();
    app_b.set_event_sink(move |ev| {
        ev_b.lock().unwrap().push(ev);
    });

    println!("--- ROUND 1: CONNECT ---");
    app_a.connect_addr("127.0.0.1", port_b).expect("connect_addr");

    let start = Instant::now();
    loop {
        let connected_a = events_a.lock().unwrap().iter().any(|e| matches!(e, LtEvent::ConnState { state, .. } if state == "connected"));
        let connected_b = events_b.lock().unwrap().iter().any(|e| matches!(e, LtEvent::ConnState { state, .. } if state == "connected"));
        if connected_a && connected_b {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        if start.elapsed() > Duration::from_secs(5) {
            panic!("Round 1 connection timeout");
        }
    }

    println!("--- ROUND 1: DISCONNECT ---");
    events_a.lock().unwrap().clear();
    events_b.lock().unwrap().clear();
    app_a.disconnect(&uuid_b).expect("disconnect");

    let start = Instant::now();
    loop {
        let disc_a = events_a.lock().unwrap().iter().any(|e| matches!(e, LtEvent::ConnState { state, .. } if state == "disconnected"));
        let disc_b = events_b.lock().unwrap().iter().any(|e| matches!(e, LtEvent::ConnState { state, .. } if state == "disconnected"));
        if disc_a && disc_b {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        if start.elapsed() > Duration::from_secs(3) {
            println!("Round 1 events A: {:?}", events_a.lock().unwrap());
            println!("Round 1 events B: {:?}", events_b.lock().unwrap());
            panic!("Round 1 disconnect timeout");
        }
    }

    for ev in events_a.lock().unwrap().iter() {
        if let LtEvent::ConnState { uuid, state, .. } = ev {
            println!("Round 1 Disconnect A got: state={}, uuid='{}'", state, uuid);
            assert_eq!(uuid, &uuid_b, "Event A on disconnect MUST contain uuid_b!");
        }
    }

    for ev in events_b.lock().unwrap().iter() {
        if let LtEvent::ConnState { uuid, state, .. } = ev {
            println!("Round 1 Disconnect B got: state={}, uuid='{}'", state, uuid);
            assert_eq!(uuid, &uuid_a, "Event B on disconnect MUST contain uuid_a!");
        }
    }

    std::thread::sleep(Duration::from_millis(300));

    println!("--- ROUND 2: CONNECT ---");
    events_a.lock().unwrap().clear();
    events_b.lock().unwrap().clear();
    app_a.connect_addr("127.0.0.1", port_b).expect("connect_addr 2");

    let start = Instant::now();
    loop {
        let connected_a = events_a.lock().unwrap().iter().any(|e| matches!(e, LtEvent::ConnState { state, .. } if state == "connected"));
        let connected_b = events_b.lock().unwrap().iter().any(|e| matches!(e, LtEvent::ConnState { state, .. } if state == "connected"));
        if connected_a && connected_b {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        if start.elapsed() > Duration::from_secs(5) {
            println!("Round 2 connect events A: {:?}", events_a.lock().unwrap());
            println!("Round 2 connect events B: {:?}", events_b.lock().unwrap());
            panic!("Round 2 connection timeout");
        }
    }

    println!("--- ROUND 2: DISCONNECT ---");
    events_a.lock().unwrap().clear();
    events_b.lock().unwrap().clear();
    app_a.disconnect(&uuid_b).expect("disconnect 2");

    let start = Instant::now();
    loop {
        let disc_a = events_a.lock().unwrap().iter().any(|e| matches!(e, LtEvent::ConnState { state, .. } if state == "disconnected"));
        let disc_b = events_b.lock().unwrap().iter().any(|e| matches!(e, LtEvent::ConnState { state, .. } if state == "disconnected"));
        if disc_a && disc_b {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        if start.elapsed() > Duration::from_secs(3) {
            println!("Round 2 events A: {:?}", events_a.lock().unwrap());
            println!("Round 2 events B: {:?}", events_b.lock().unwrap());
            panic!("Round 2 disconnect timeout");
        }
    }

    for ev in events_a.lock().unwrap().iter() {
        if let LtEvent::ConnState { uuid, state, .. } = ev {
            println!("Round 2 Disconnect A got: state={}, uuid='{}'", state, uuid);
            assert_eq!(uuid, &uuid_b, "Event A on disconnect 2 MUST contain uuid_b!");
        }
    }

    for ev in events_b.lock().unwrap().iter() {
        if let LtEvent::ConnState { uuid, state, .. } = ev {
            println!("Round 2 Disconnect B got: state={}, uuid='{}'", state, uuid);
            assert_eq!(uuid, &uuid_a, "Event B on disconnect 2 MUST contain uuid_a!");
        }
    }

    app_a.shutdown();
    app_b.shutdown();
    let _ = std::fs::remove_dir_all(&temp_root);
}
