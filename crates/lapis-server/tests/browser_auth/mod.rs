use super::support::Fixture;
mod database;
mod guards;
mod ownership;
mod support;

use lapis_app_protocol::{PROTOCOL_MAJOR, browser::SessionInfo};
use lapis_server::http::VERSION_HEADER;
use support::BrowserFixture;

#[tokio::test]
async fn hosted_info_is_public_and_reports_the_coordinated_protocol() {
    let f = Fixture::start().await;
    let reply = f
        .client
        .get(format!("{}/api/v3/system/info", f.base))
        .send()
        .await
        .unwrap();
    assert_eq!(reply.status(), 200);
    let info: lapis_app_protocol::browser::HostInfo = reply.json().await.unwrap();
    assert_eq!(info.protocol_major, 3);
    assert!(!info.browser_auth);
    f.host.shutdown().unwrap();
}

#[tokio::test]
async fn cookie_login_current_logout_and_secure_flags_preserve_existing_controls() {
    for insecure in [true, false] {
        let mut f = BrowserFixture::start(insecure, None).await;
        let (cookie, info, header) = f.login().await;
        assert_eq!(info.username, "Owner");
        for flag in ["Path=/", "HttpOnly", "SameSite=Lax", "Max-Age=604800"] {
            assert!(header.contains(flag), "{header}");
        }
        assert!(!header.contains("Domain="));
        assert_eq!(header.contains("; Secure"), !insecure);
        assert_eq!(cookie.starts_with("__Host-lapis_session="), !insecure);
        let current = f
            .authorize(
                f.client.get(format!("{}/api/v3/auth/current", f.base)),
                &cookie,
                &info,
                false,
            )
            .send()
            .await
            .unwrap();
        assert_eq!(current.status(), 200);
        assert_eq!(current.headers()["cache-control"], "no-store");
        let current: SessionInfo = current.json().await.unwrap();
        assert_eq!(current.csrf_token, info.csrf_token);
        let logged_out = f
            .authorize(
                f.client.post(format!("{}/api/v3/auth/logout", f.base)),
                &cookie,
                &info,
                true,
            )
            .send()
            .await
            .unwrap();
        assert_eq!(logged_out.status(), 200);
        assert!(
            logged_out.headers()["set-cookie"]
                .to_str()
                .unwrap()
                .contains("Max-Age=0")
        );
        let refused = f
            .client
            .get(format!("{}/api/v3/auth/current", f.base))
            .header("cookie", &cookie)
            .header(VERSION_HEADER, PROTOCOL_MAJOR)
            .send()
            .await
            .unwrap();
        assert_eq!(refused.status(), 401);
        f.finish().await;
    }
}
