use std::net::{IpAddr, ToSocketAddrs};

use thiserror::Error;
use url::Url;

/// URL Ollama normalisée (schéma HTTP(S), sans credentials/fragment/query).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedOllamaUrl {
    url: Url,
    remote: bool,
}

impl NormalizedOllamaUrl {
    pub fn as_str(&self) -> &str {
        self.url.as_str().trim_end_matches('/')
    }

    pub fn is_remote(&self) -> bool {
        self.remote
    }

    pub fn into_string(self) -> String {
        self.as_str().to_string()
    }
}

impl std::fmt::Display for NormalizedOllamaUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OllamaUrlError {
    #[error("L'URL Ollama est vide.")]
    Empty,

    #[error("URL Ollama invalide.")]
    Invalid,

    #[error("Seul le schéma HTTP ou HTTPS est autorisé pour Ollama.")]
    UnsupportedScheme,

    #[error("Les identifiants dans l'URL Ollama sont interdits.")]
    Credentials,

    #[error("Les fragments (#…) sont interdits dans l'URL Ollama.")]
    Fragment,

    #[error("Les paramètres de requête sont interdits dans l'URL Ollama.")]
    Query,

    #[error("Les destinations link-local ou metadata sont interdites.")]
    LinkLocalOrMetadata,

    #[error("Un serveur Ollama distant ou LAN nécessite une autorisation explicite.")]
    RemoteNotAllowed,

    #[error("L'hôte Ollama est invalide ou introuvable.")]
    Host,
}

/// Parse, normalise et classe une URL Ollama.
///
/// Loopback autorisé par défaut ; LAN / distant uniquement si `allow_remote`.
/// Link-local / metadata toujours refusés.
pub fn normalize(raw: &str, allow_remote: bool) -> Result<NormalizedOllamaUrl, OllamaUrlError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(OllamaUrlError::Empty);
    }

    let mut url = Url::parse(trimmed).map_err(|_| OllamaUrlError::Invalid)?;

    match url.scheme() {
        "http" | "https" => {}
        _ => return Err(OllamaUrlError::UnsupportedScheme),
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err(OllamaUrlError::Credentials);
    }
    if url.fragment().is_some() {
        return Err(OllamaUrlError::Fragment);
    }
    if url.query().is_some() {
        return Err(OllamaUrlError::Query);
    }
    if url.host().is_none() {
        return Err(OllamaUrlError::Host);
    }

    // Base API : chemin vide ou « / » uniquement (pas de proxy path arbitraire).
    match url.path() {
        "" | "/" => {
            url.set_path("");
        }
        _ => return Err(OllamaUrlError::Invalid),
    }

    let host = url.host_str().ok_or(OllamaUrlError::Host)?;
    let port = url.port_or_known_default().unwrap_or(80);
    let ips = resolve_ips(host, port)?;
    let remote = classify_remote(&ips)?;

    if remote && !allow_remote {
        return Err(OllamaUrlError::RemoteNotAllowed);
    }

    Ok(NormalizedOllamaUrl { url, remote })
}

/// Cible de redirection acceptable (même contrôles réseau, sans exiger l'opt-in remote
/// — l'origine a déjà été validée ; on bloque seulement link-local / schémas / credentials).
pub fn is_allowed_redirect_target(url: &Url) -> bool {
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    if url.fragment().is_some() || url.query().is_some() {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let port = url.port_or_known_default().unwrap_or(80);
    match resolve_ips(host, port) {
        Ok(ips) => classify_remote(&ips).is_ok(),
        Err(_) => false,
    }
}

pub fn same_origin(a: &Url, b: &Url) -> bool {
    a.scheme() == b.scheme()
        && a.host() == b.host()
        && a.port_or_known_default() == b.port_or_known_default()
}

fn resolve_ips(host: &str, port: u16) -> Result<Vec<IpAddr>, OllamaUrlError> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![ip]);
    }

    let addrs = (host, port)
        .to_socket_addrs()
        .map_err(|_| OllamaUrlError::Host)?;
    let ips: Vec<IpAddr> = addrs.map(|addr| addr.ip()).collect();
    if ips.is_empty() {
        return Err(OllamaUrlError::Host);
    }
    Ok(ips)
}

/// Retourne `true` si au moins une IP n'est pas loopback (LAN / public).
fn classify_remote(ips: &[IpAddr]) -> Result<bool, OllamaUrlError> {
    if ips.is_empty() {
        return Err(OllamaUrlError::Host);
    }

    let mut remote = false;
    for ip in ips {
        if is_forbidden_ip(*ip) {
            return Err(OllamaUrlError::LinkLocalOrMetadata);
        }
        if !ip.is_loopback() {
            remote = true;
        }
    }
    Ok(remote)
}

fn is_forbidden_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_unspecified() || v4.is_broadcast() || v4.is_multicast() || v4.is_link_local()
            // 169.254.0.0/16 (inclut metadata 169.254.169.254)
        }
        IpAddr::V6(v6) => {
            v6.is_unspecified() || v6.is_multicast() || v6.is_unicast_link_local()
            // fe80::/10
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_loopback_default() {
        let normalized = normalize("http://127.0.0.1:11434", false).expect("loopback");
        assert_eq!(normalized.as_str(), "http://127.0.0.1:11434");
        assert!(!normalized.is_remote());
    }

    #[test]
    fn accepts_localhost_and_strips_trailing_slash() {
        let normalized = normalize(" http://localhost:11434/ ", false).expect("localhost");
        assert_eq!(normalized.as_str(), "http://localhost:11434");
        assert!(!normalized.is_remote());
    }

    #[test]
    fn rejects_empty_and_whitespace() {
        assert_eq!(normalize("", false), Err(OllamaUrlError::Empty));
        assert_eq!(normalize("   \t", false), Err(OllamaUrlError::Empty));
    }

    #[test]
    fn rejects_file_scheme() {
        assert_eq!(
            normalize("file:///etc/passwd", false),
            Err(OllamaUrlError::UnsupportedScheme)
        );
    }

    #[test]
    fn rejects_credentials_and_fragment() {
        assert_eq!(
            normalize("http://user:pass@127.0.0.1:11434", false),
            Err(OllamaUrlError::Credentials)
        );
        assert_eq!(
            normalize("http://127.0.0.1:11434/#frag", false),
            Err(OllamaUrlError::Fragment)
        );
    }

    #[test]
    fn rejects_link_local_and_metadata() {
        assert_eq!(
            normalize("http://169.254.169.254/", true),
            Err(OllamaUrlError::LinkLocalOrMetadata)
        );
        assert_eq!(
            normalize("http://169.254.1.1:11434", true),
            Err(OllamaUrlError::LinkLocalOrMetadata)
        );
    }

    #[test]
    fn lan_requires_opt_in() {
        assert_eq!(
            normalize("http://192.168.1.10:11434", false),
            Err(OllamaUrlError::RemoteNotAllowed)
        );
        let normalized = normalize("http://192.168.1.10:11434", true).expect("lan");
        assert!(normalized.is_remote());
        assert_eq!(normalized.as_str(), "http://192.168.1.10:11434");
    }

    #[test]
    fn rejects_non_root_path_and_query() {
        assert_eq!(
            normalize("http://127.0.0.1:11434/api", false),
            Err(OllamaUrlError::Invalid)
        );
        assert_eq!(
            normalize("http://127.0.0.1:11434?x=1", false),
            Err(OllamaUrlError::Query)
        );
    }
}
