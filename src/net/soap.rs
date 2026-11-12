//! Minimal UPnP/SOAP client for the speakers' local control services (port 1400).
//! S1 and S2 firmware expose the same services, so one client drives both.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

pub type Service = (&'static str, &'static str);

pub const AV_TRANSPORT: Service = ("/MediaRenderer/AVTransport/Control", "urn:schemas-upnp-org:service:AVTransport:1");
pub const RENDERING: Service = ("/MediaRenderer/RenderingControl/Control", "urn:schemas-upnp-org:service:RenderingControl:1");
pub const GROUP_RENDERING: Service = ("/MediaRenderer/GroupRenderingControl/Control", "urn:schemas-upnp-org:service:GroupRenderingControl:1");
pub const CONTENT_DIRECTORY: Service = ("/MediaServer/ContentDirectory/Control", "urn:schemas-upnp-org:service:ContentDirectory:1");
pub const ZONE_TOPOLOGY: Service = ("/ZoneGroupTopology/Control", "urn:schemas-upnp-org:service:ZoneGroupTopology:1");
pub const ALARM_CLOCK: Service = ("/AlarmClock/Control", "urn:schemas-upnp-org:service:AlarmClock:1");
pub const DEVICE_PROPS: Service = ("/DeviceProperties/Control", "urn:schemas-upnp-org:service:DeviceProperties:1");

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(3))
            .timeout(Duration::from_secs(8))
            // Fresh connections every time: pooled ones go stale when the computer sleeps.
            .max_idle_connections(0)
            .build()
    })
}

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Call one action and return the response's output arguments by name.
pub fn call(ip: &str, service: Service, action: &str, args: &[(&str, &str)]) -> Result<HashMap<String, String>, String> {
    let mut inner = String::new();
    for (k, v) in args {
        inner.push_str(&format!("<{k}>{}</{k}>", escape(v)));
    }
    let body = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
<s:Body><u:{action} xmlns:u=\"{}\">{inner}</u:{action}></s:Body></s:Envelope>",
        service.1
    );
    let url = format!("http://{ip}:1400{}", service.0);
    let result = agent().post(&url).set("Content-Type", "text/xml; charset=\"utf-8\"").set("SOAPACTION", &format!("\"{}#{}\"", service.1, action)).send_string(&body);
    match result {
        Ok(resp) => {
            let text = resp.into_string().map_err(|e| format!("{action}: {e}"))?;
            parse_response(&text, action)
        }
        Err(ureq::Error::Status(code, resp)) => {
            let text = resp.into_string().unwrap_or_default();
            let upnp = text.split("<errorCode>").nth(1).and_then(|s| s.split('<').next()).map(|c| format!(", code {c}")).unwrap_or_default();
            Err(format!("{action} was refused (HTTP {code}{upnp})"))
        }
        Err(e) => Err(format!("{action}: {e}")),
    }
}

fn parse_response(text: &str, action: &str) -> Result<HashMap<String, String>, String> {
    let doc = roxmltree::Document::parse(text).map_err(|e| format!("{action}: bad reply ({e})"))?;
    let wanted = format!("{action}Response");
    let node = doc.descendants().find(|n| n.tag_name().name() == wanted).ok_or_else(|| format!("{action}: unexpected reply"))?;
    let mut map = HashMap::new();
    for child in node.children().filter(|c| c.is_element()) {
        let text: String = child.children().filter(|c| c.is_text()).filter_map(|c| c.text()).collect();
        map.insert(child.tag_name().name().to_string(), text);
    }
    Ok(map)
}

pub fn get_text(url: &str) -> Result<String, String> {
    agent().get(url).call().map_err(|e| e.to_string())?.into_string().map_err(|e| e.to_string())
}

pub fn get_bytes(url: &str) -> Result<Vec<u8>, String> {
    let resp = agent().get(url).call().map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    use std::io::Read;
    resp.into_reader().take(8 * 1024 * 1024).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}
