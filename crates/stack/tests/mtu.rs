use zero_stack::tcp_mss_for_mtu;

#[test]
fn derives_tcp_mss_from_configured_mtu() {
    assert_eq!(tcp_mss_for_mtu(1500), 1440);
    assert_eq!(tcp_mss_for_mtu(1400), 1340);
    assert_eq!(tcp_mss_for_mtu(1280), 1220);
}
