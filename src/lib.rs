pub mod client;

use client::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[repr(u8)]
#[derive(Deserialize, Debug, PartialEq, Clone, Copy)]
pub enum Trend {
    None,
    DoubleUp,
    SingleUp,
    FortyFiveUp,
    Flat,
    FortyFiveDown,
    SingleDown,
    DoubleDown,
    NotComputable,
    RateOutOfRange,
}

#[repr(u8)]
#[derive(Debug, PartialEq)]
pub enum DexcomError {
    AccountPasswordInvalid,
    AuthenticateMaxAttempsExceed,
    SessionNotFound,
    SessionInvalid,
    InvalidUsername,
    InvalidPassword,
    InvalidAccountId,
    InvalidUnknown,
    Unknown,
}

pub struct Dexcom<'a, C: Client> {
    client: &'a mut C,
}

#[derive(Serialize)]
struct GetLatestGlucoseValuesRequest<'a> {
    #[serde(rename = "sessionId")]
    session_id: &'a str,
    minutes: u32,
    #[serde(rename = "maxCount")]
    max_count: u32,
}

#[derive(Serialize)]
struct GetAccountIdRequest<'a> {
    #[serde(rename = "accountName")]
    account_name: &'a str,
    password: &'a str,
    #[serde(rename = "applicationId")]
    application_id: &'a str,
}

#[derive(Serialize)]
struct GetSessionIdRequest<'a> {
    #[serde(rename = "accountId")]
    account_id: &'a str,
    password: &'a str,
    #[serde(rename = "applicationId")]
    application_id: &'a str,
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Deserialize, Debug, Clone, Copy)]
pub struct GlucosReading {
    #[serde(rename = "Value")]
    pub value: i32,
    #[serde(rename = "Trend")]
    pub trend: Trend,
}

#[derive(Deserialize)]
struct DexcomErrorResponse<'a> {
    #[serde(rename = "Code")]
    code: Option<&'a str>,
    message: Option<&'a str>,
}

impl From<DexcomErrorResponse<'_>> for DexcomError {
    fn from(val: DexcomErrorResponse<'_>) -> Self {
        use DexcomError::*;
        match val.code {
            None => Unknown,
            Some(code) => match code {
                "SessionIdNotFound" => SessionNotFound,
                "SessionNotValid" => SessionInvalid,
                "AccountPasswordInvalid" => AccountPasswordInvalid,
                "SSO_AuthenticateMaxAttemptsExceeed" => AuthenticateMaxAttempsExceed,
                "InvalidArgument" => {
                    match val.message {
                        None => InvalidUnknown,
                        Some(message) => {
                            if message.contains("accountName") {
                                InvalidUsername
                            } else if message.contains("password") {
                                InvalidPassword
                            } else if message.contains("UUID") {
                                InvalidAccountId
                            } else {
                                InvalidUnknown
                            }
                        }
                    }
                },
                _ => Unknown,
            },
        }
    }
}

#[derive(Debug)]
pub struct SerdeJsonError(pub serde_json::Error);

impl From<serde_json::Error> for SerdeJsonError {
    fn from(value: serde_json::Error) -> Self {
        SerdeJsonError(value)
    }
}

#[derive(Debug)]
pub enum ClientError<E: embedded_svc::io::Error> {
    ConnectionError(E),
    DexcomError(DexcomError),
    JSONError(SerdeJsonError),
}

impl<E: embedded_svc::io::Error> From<SerdeJsonError> for ClientError<E> {
    fn from(value: SerdeJsonError) -> Self {
        ClientError::JSONError(value)
    }
}

impl<E: embedded_svc::io::Error> From<E> for ClientError<E> {
    fn from(value: E) -> Self {
        ClientError::ConnectionError(value)
    }
}

type Result<T, C> = std::result::Result<T, ClientError<<C as Client>::Error>>;

impl<'a, C: Client> Dexcom<'a, C> {
    pub fn new(client: &'a mut C) -> Self {
        Self { client }
    }

    fn post_request<S: Serialize, D: DeserializeOwned>(
        &mut self,
        uri: &str,
        request: &S,
    ) -> Result<D, C> {
        let body = serde_json::to_vec(&request).map_err(SerdeJsonError)?;
        let mut buf = [0; 512];

        let (size, status_code) = self.client.post_request(
            uri,
            &[
                ("Content-Type", "application/json"),
                ("User-Agent", "rsdexcom/0.0.1"),
            ],
            &body,
            &mut buf,
        )?;

        let buf = &buf[..size];

        #[cfg(feature = "log")]
        log::info!("{:?}", String::from_utf8(buf.to_vec()));

        match status_code {
            200..=299 => {
                let response = serde_json::from_slice::<D>(buf).map_err(SerdeJsonError)?;
                Ok(response)
            }
            _ => {
                let response = serde_json::from_slice::<DexcomErrorResponse>(buf)
                    .map_err(SerdeJsonError)?;
                let error: DexcomError = response.into();
                Err(ClientError::DexcomError(error))
            }
        }
    }

    pub fn get_current_glucose_reading(
        &mut self,
        session_id: &str,
    ) -> Result<GlucosReading, C> {
        let reading = self.post_request::<_, [GlucosReading; 1]>(
            url::DEXCOM_GLUCOSE_READINGS_ENDPOINT,
            &GetLatestGlucoseValuesRequest {
                session_id,
                minutes: 10,
                max_count: 1,
            },
        )?;

        Ok(reading[0])
    }

    pub fn load_session_id(
        &mut self,
        account_name: &str,
        password: &str,
        application_id: &str,
    ) -> Result<String, C> {
        let account_id = self.get_account_id(account_name, password, application_id)?;
        let session_id = self.get_session_id(&account_id, password, application_id)?;
        Ok(session_id)
    }

    fn get_account_id(
        &mut self,
        account_name: &str,
        password: &str,
        application_id: &str,
    ) -> Result<String, C> {
        self.post_request(
            url::DEXCOM_AUTHENTICATE_ENDPOINT,
            &GetAccountIdRequest {
                account_name,
                password,
                application_id,
            },
        )
    }

    fn get_session_id(
        &mut self,
        account_id: &str,
        password: &str,
        application_id: &str,
    ) -> Result<String, C> {
        self.post_request(
            url::DEXCOM_LOGIN_ID_ENDPOINT,
            &GetSessionIdRequest {
                account_id,
                password,
                application_id,
            },
        )
    }
}

#[cfg(feature = "ous")]
mod url {
    pub(crate) const DEXCOM_GLUCOSE_READINGS_ENDPOINT: &str = 
        "https://shareous1.dexcom.com/ShareWebServices/Services/Publisher/ReadPublisherLatestGlucoseValues";
    pub(crate) const DEXCOM_LOGIN_ID_ENDPOINT: &str =
        "https://shareous1.dexcom.com/ShareWebServices/Services/General/LoginPublisherAccountById";
    pub(crate) const DEXCOM_AUTHENTICATE_ENDPOINT: &str =
        "https://shareous1.dexcom.com/ShareWebServices/Services/General/AuthenticatePublisherAccount";
}

#[cfg(not(feature = "ous"))]
mod url {
    pub(crate) const DEXCOM_GLUCOSE_READINGS_ENDPOINT: &str = 
        "https://share2.dexcom.com/ShareWebServices/Services/Publisher/ReadPublisherLatestGlucoseValues";
    pub(crate) const DEXCOM_LOGIN_ID_ENDPOINT: &str =
        "https://share2.dexcom.com/ShareWebServices/Services/General/LoginPublisherAccountById";
    pub(crate) const DEXCOM_AUTHENTICATE_ENDPOINT: &str =
        "https://share2.dexcom.com/ShareWebServices/Services/General/AuthenticatePublisherAccount";    
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use mockall::predicate::*;

    use super::*;
    use super::client::*;

    #[test]
    fn test_get_current_glucose_reading() {
        let mut client = MockClient::new();

        client
            .expect_post_request()
            .with(
                eq(url::DEXCOM_AUTHENTICATE_ENDPOINT),
                always(),
                always(),
                always(),
            )
            .returning(|_, _, _, mut buf| {
                let size = buf
                    .write(b"\"1e913fce-5a34-4d27-a991-b6cb3a3bd3d8\"")
                    .unwrap();
                Ok((size, 200u16))
            });

        client
            .expect_post_request()
            .with(
                eq(url::DEXCOM_LOGIN_ID_ENDPOINT),
                always(),
                always(),
                always(),
            )
            .returning(|_, _, _, mut buf| {
                let size = buf
                    .write(b"\"a21d18db-a276-40bc-8337-77dcd02df53e\"")
                    .unwrap();
                Ok((size, 200u16))
            });

        client
            .expect_post_request()
            .with(
                eq(url::DEXCOM_GLUCOSE_READINGS_ENDPOINT),
                always(),
                always(),
                always(),
            )
            .returning(|_, _, _, mut buf| {
                let size = buf.write(r#"[{"WT":"Date(1699110415000)","ST":"Date(1699110415000)","DT":"Date(1699110415000+0900)","Value":153,"Trend":"Flat"}]"#.as_bytes()).unwrap();
                Ok((size, 200u16))
            });

        let mut dexcom = Dexcom::new(&mut client);

        let session_id = dexcom.load_session_id("", "", "").unwrap();
        assert_eq!(session_id, "a21d18db-a276-40bc-8337-77dcd02df53e");

        let glucose = dexcom.get_current_glucose_reading(&session_id);

        assert!(glucose.is_ok());
        assert_eq!(
            glucose.unwrap(),
            GlucosReading {
                trend: Trend::Flat,
                value: 153,
            }
        )
    }

    #[test]
    fn test_dexcom_error_response() {
        let message = r#"{"Code":"SessionIdNotFound"}"#;
        let response = serde_json::from_str::<DexcomErrorResponse>(message).unwrap();

        let error: DexcomError = response.into();
        assert_eq!(error, DexcomError::SessionNotFound);
    }
}
