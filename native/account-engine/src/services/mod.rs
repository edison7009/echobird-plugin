pub mod account_login_browser;
pub mod account_model_sources;
pub mod account_proxy;
pub mod claude_code_accounts;
pub mod claude_code_oauth;
pub mod claude_desktop_accounts;
pub mod claude_desktop_history;
pub mod cursor_auth;
pub mod devin_protocol;
pub mod doubao_accounts;
pub mod electron_storage;
pub mod model_manager;
pub mod native_account_files;
pub mod process_manager;
pub mod tool_manager;
pub mod workbuddy_accounts;

pub mod smart_router {
    pub fn forget_account_authentication(
        provider: crate::models::model::AccountProvider,
        id: &str,
    ) {
        super::account_proxy::forget_preparation_metadata(provider, id);
    }
}

pub mod msix {
    pub fn roaming_data_dir(_uri: &str, _product: &str) -> Option<std::path::PathBuf> {
        super::tool_manager::context()["claudedesktop"]["roamingDataDir"]
            .as_str()
            .map(std::path::PathBuf::from)
    }
}

#[cfg(test)]
mod anthropic_conversion;
mod conversion;
pub mod local_llm {
    pub mod proxy {
        #[cfg(test)]
        pub(crate) use super::super::anthropic_conversion::anthropic_to_openai;
        pub(crate) use super::super::conversion::openai_to_anthropic;
    }
}

// This adapter only uses the shared protocol helpers, not another account provider.
pub mod opendesign_accounts {
    pub const LINK: &str = "";
}
