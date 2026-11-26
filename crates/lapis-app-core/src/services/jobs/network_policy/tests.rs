use super::*;
use serde_json::json;

#[test]
fn special_addresses_stay_denied_even_with_private_approval() {
    for value in [
        "0.0.0.0",
        "169.254.169.254",
        "169.254.170.2",
        "100.100.100.200",
        "198.18.0.1",
        "192.0.2.1",
        "198.51.100.1",
        "203.0.113.2",
        "224.0.0.1",
        "255.255.255.255",
        "::",
        "fe80::1",
        "ff02::1",
        "2001:db8::1",
        "2002:7f00:1::",
        "::ffff:169.254.169.254",
        "64:ff9b::7f00:1",
    ] {
        assert!(!address(value.parse().unwrap(), true, true), "{value}");
    }
    for value in [
        "127.0.0.1",
        "10.0.0.1",
        "172.16.0.1",
        "192.168.1.1",
        "::1",
        "fd00::1",
        "::ffff:127.0.0.1",
    ] {
        let ip = value.parse().unwrap();
        assert!(!address(ip, false, true));
        assert!(address(ip, true, false));
    }
    for value in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
        let ip = value.parse().unwrap();
        assert!(address(ip, false, true));
        assert!(!address(ip, true, false));
    }
}

fn grants() -> Vec<JobNetworkGrant> {
    serde_json::from_value(json!([{"origin":"https://bank.example","methods":["GET","POST"],"allowPrivateNetwork":false,
        "credentials":[{"id":"bank","header":"authorization","prefix":"Bearer "}]}])).unwrap()
}

#[test]
fn origin_methods_headers_and_binding_are_independently_scoped() {
    let grants = grants();
    for input in [
        json!({"url":"https://bank.example/transactions","credential":"other"}),
        json!({"url":"https://evil.example/","credential":"bank"}),
        json!({"url":"https://bank.example:444/"}),
        json!({"url":"http://bank.example/"}),
        json!({"url":"https://user:secret@bank.example/"}),
        json!({"url":"https://bank.example/#fragment"}),
        json!({"url":"https://bank.example/","method":"DELETE"}),
        json!({"url":"https://bank.example/","headers":{"Host":"evil.example"}}),
        json!({"url":"https://bank.example/","headers":{"Cookie":"ambient"}}),
        json!({"url":"https://bank.example/","headers":{"Authorization":"secret"}}),
        json!({"url":"https://bank.example/","headers":{"content-type":"text/plain\r\nHost: evil.example"}}),
        json!({"url":"https://bank.example/","body":"unapproved GET body"}),
        json!({"url":"https://bank.example/","method":"POST","body":"x".repeat(65537)}),
    ] {
        let request: Request = serde_json::from_value(input).unwrap();
        assert!(grant(&request, &grants).is_err());
    }
    let request: Request = serde_json::from_value(json!({"url":"https://bank.example/transactions","credential":"bank","headers":{"accept":"application/json"}})).unwrap();
    assert!(grant(&request, &grants).is_ok());
}

#[test]
fn declarations_reject_noncanonical_or_ambiguous_authority() {
    for origin in [
        "https://bank.example/",
        "https://bank.example/path",
        "https://BANK.example",
        "https://bank.example:443",
        "https://user@bank.example",
        "file:///tmp/a",
        "http://bank.example",
    ] {
        let mut grants = grants();
        grants[0].origin = origin.into();
        assert!(validate(&grants).is_err(), "{origin}");
    }
    let mut g = grants();
    g.push(g[0].clone());
    assert!(validate(&g).is_err());
    let mut g = grants();
    g[0].credentials[0].header = "host".into();
    assert!(validate(&g).is_err());
}
