use std::time::Duration;
use ureq::tls::{RootCerts, TlsConfig};
use ureq::{Agent, Error};

pub fn platform_agent(timeout: Duration) -> Agent {
    Agent::config_builder()
        .timeout_global(Some(timeout))
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into()
}

pub fn with_retry<T>(mut operation: impl FnMut() -> Result<T, Error>) -> Result<T, Error> {
    for attempt in 0..3 {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) if attempt < 2 && is_retryable(&error) => {
                std::thread::sleep(Duration::from_millis(if attempt == 0 { 250 } else { 750 }));
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("retry loop always returns")
}

fn is_retryable(error: &Error) -> bool {
    matches!(
        error,
        Error::StatusCode(500 | 502 | 503 | 504)
            | Error::Io(_)
            | Error::Timeout(_)
            | Error::HostNotFound
            | Error::ConnectionFailed
    )
}

pub fn github_api_error(error: Error, feature: &str) -> String {
    match error {
        Error::StatusCode(403) => {
            "GitHub 公开接口已被限流 (未登录每小时约 60 次), 请稍后或更换网络再试".to_string()
        }
        Error::StatusCode(429) => "GitHub 请求过于频繁, 请稍后重试".to_string(),
        Error::StatusCode(code) => format!("GitHub {feature}返回 HTTP {code}"),
        other => format!("连接 GitHub {feature}失败: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_transient_server_errors() {
        let mut attempts = 0;
        let value = with_retry(|| {
            attempts += 1;
            if attempts < 3 {
                Err(Error::StatusCode(502))
            } else {
                Ok("ok")
            }
        })
        .unwrap();
        assert_eq!(value, "ok");
        assert_eq!(attempts, 3);
    }

    #[test]
    fn does_not_retry_permanent_client_errors() {
        let mut attempts = 0;
        let error = with_retry::<()>(|| {
            attempts += 1;
            Err(Error::StatusCode(404))
        })
        .unwrap_err();
        assert!(matches!(error, Error::StatusCode(404)));
        assert_eq!(attempts, 1);
    }

    #[test]
    fn describes_unauthenticated_rate_limit_separately_from_network_errors() {
        let limited = github_api_error(Error::StatusCode(403), "项目接口");
        let busy = github_api_error(Error::StatusCode(429), "版本接口");
        let http = github_api_error(Error::StatusCode(500), "市场接口");
        assert!(limited.contains("限流"));
        assert!(limited.contains("60"));
        assert!(busy.contains("过于频繁"));
        assert_eq!(http, "GitHub 市场接口返回 HTTP 500");
        assert_eq!(
            github_api_error(Error::StatusCode(403), "版本接口"),
            github_api_error(Error::StatusCode(403), "项目接口")
        );
    }
}
