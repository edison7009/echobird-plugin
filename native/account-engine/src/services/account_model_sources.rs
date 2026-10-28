use super::{claude_code_accounts as code, doubao_accounts as doubao, workbuddy_accounts as work};
use crate::models::model::{AccountProvider, AccountSource};
use serde_json::Value;

pub(crate) fn credential_error(message: String) -> super::account_proxy::ProxyError {
    super::account_proxy::ProxyError::new(axum::http::StatusCode::UNAUTHORIZED, message)
}

pub async fn credentials(source: &AccountSource, force: bool) -> Result<Value, String> {
    use AccountProvider::*;
    match source.provider {
        ClaudeCode => code::proxy_credentials(&source.account_id, force).await,
        WorkBuddy | WorkBuddyAi => {
            work::proxy_credentials(
                if source.provider == WorkBuddy {
                    work::Edition::Cn
                } else {
                    work::Edition::Ai
                },
                &source.account_id,
                force,
            )
            .await
        }
        DoubaoWork | Dola => {
            doubao::proxy_credentials(
                if source.provider == DoubaoWork {
                    doubao::Edition::DoubaoWork
                } else {
                    doubao::Edition::Dola
                },
                &source.account_id,
            )
            .await
        }
        _ => Err("accountError.unavailable".into()),
    }
}

pub async fn credentials_after_rejection(
    source: &AccountSource,
    rejected: &Value,
) -> Result<Value, String> {
    use AccountProvider::*;
    match source.provider {
        ClaudeCode => code::proxy_credentials_after_rejection(&source.account_id, rejected).await,
        WorkBuddy | WorkBuddyAi => {
            work::proxy_credentials_after_rejection(
                if source.provider == WorkBuddy {
                    work::Edition::Cn
                } else {
                    work::Edition::Ai
                },
                &source.account_id,
                rejected,
            )
            .await
        }
        DoubaoWork | Dola => {
            let value = credentials(source, false).await?;
            if value["cookieHeader"] == rejected["cookieHeader"] {
                Err("accountError.loginRequired".into())
            } else {
                Ok(value)
            }
        }
        _ => Err("accountError.unavailable".into()),
    }
}
