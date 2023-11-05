#[cfg(feature = "ous")]
pub const DEXCOM_BASE_DOMAIN: &str = "https://shareous1.dexcom.com";
#[cfg(feature = "ous")]
pub(crate) const DEXCOM_GLUCOSE_READINGS_ENDPOINT: &str = 
    "https://shareous1.dexcom.com/ShareWebServices/Services/Publisher/ReadPublisherLatestGlucoseValues";
#[cfg(feature = "ous")]
pub(crate) const DEXCOM_LOGIN_ID_ENDPOINT: &str =
    "https://shareous1.dexcom.com/ShareWebServices/Services/General/LoginPublisherAccountById";
#[cfg(feature = "ous")]
pub(crate) const DEXCOM_AUTHENTICATE_ENDPOINT: &str =
    "https://shareous1.dexcom.com/ShareWebServices/Services/General/AuthenticatePublisherAccount";

#[cfg(not(feature = "ous"))]
pub const DEXCOM_BASE_DOMAIN: &str = "https://share2.dexcom.com/ShareWebServices/Services";
#[cfg(not(feature = "ous"))]
pub(crate) const DEXCOM_GLUCOSE_READINGS_ENDPOINT: &str = 
    "https://share2.dexcom.com/ShareWebServices/Services/Publisher/ReadPublisherLatestGlucoseValues";
#[cfg(not(feature = "ous"))]
pub(crate) const DEXCOM_LOGIN_ID_ENDPOINT: &str =
    "https://share2.dexcom.com/ShareWebServices/Services/General/LoginPublisherAccountById";
#[cfg(not(feature = "ous"))]
pub(crate) const DEXCOM_AUTHENTICATE_ENDPOINT: &str =
    "https://share2.dexcom.com/ShareWebServices/Services/General/AuthenticatePublisherAccount";
