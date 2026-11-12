//! Talking to the speakers on the local network: finding them (`discovery`), the UPnP/SOAP
//! calls they answer on port 1400 (`soap`), the actions this app performs (`speaker`) and
//! the DIDL-Lite metadata they use for tracks and lists (`didl`).

pub mod didl;
pub mod discovery;
pub mod soap;
pub mod speaker;
