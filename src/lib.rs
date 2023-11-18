pub mod url;

use std::fmt::Display;

use embedded_svc::{
    http::client::{Client, Connection},
    io::ErrorType,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[repr(u8)]
#[derive(Deserialize, Debug, PartialEq)]
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

impl Display for Trend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Trend::None => "-",
            Trend::DoubleUp => "↑↑",
            Trend::SingleUp => "↑",
            Trend::FortyFiveUp => "↗",
            Trend::Flat => "→",
            Trend::FortyFiveDown => "↘",
            Trend::SingleDown => "↓",
            Trend::DoubleDown => "↓↓",
            Trend::NotComputable => "?",
            Trend::RateOutOfRange => "!",
        })
    }
}

#[repr(u8)]
#[derive(Debug, PartialEq)]
pub enum DexcomError {
    AccountError,
    SessionError,
    ArgumentError,
    UnknownError,
}

pub struct Dexcom<'a, C: Connection> {
    client: &'a mut Client<C>,
}

#[derive(Serialize)]
struct GetLatestGlucoseValuesRequest<'a> {
    #[serde(rename = "sessionId")]
    session_id: &'a str,
    // fixed as 10
    minutes: u32,
    // fixed as 1.
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
#[derive(Deserialize, Debug)]
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
    // message: Option<&'a [u8]>,
}

impl Into<DexcomError> for DexcomErrorResponse<'_> {
    fn into(self) -> DexcomError {
        match self.code {
            None => DexcomError::UnknownError,
            Some(code) => match code {
                "SessionIdNotFound" | "SessionNotValid" => DexcomError::SessionError,
                "AccountPasswordInvalid" | "SSO_AuthenticateMaxAttemptsExceeed" => {
                    DexcomError::AccountError
                }
                "InvalidArgument" => DexcomError::ArgumentError,
                _ => DexcomError::UnknownError,
            },
        }
    }
}

pub enum ClientError<E: std::error::Error> {
    ConnectionError(E),
    DexcomError(DexcomError),
    JSONError(serde_json::Error),
}

impl<E: std::error::Error> From<DexcomError> for ClientError<E> {
    fn from(value: DexcomError) -> Self {
        ClientError::DexcomError(value)
    }
}

impl<E: std::error::Error> From<serde_json::Error> for ClientError<E> {
    fn from(value: serde_json::Error) -> Self {
        ClientError::JSONError(value)
    }
}

type Result<T, C> = std::result::Result<T, ClientError<<C as ErrorType>::Error>>;

impl<'a, C: Connection> Dexcom<'a, C> {
    pub fn new(client: &'a mut Client<C>) -> Self {
        Self { client }
    }

    fn post_request<S: Serialize, D: DeserializeOwned>(
        &mut self,
        uri: &str,
        request: &S,
    ) -> Result<D, C> {
        let body = serde_json::to_vec(&request)?;

        let mut request = self
            .client
            .request(
                embedded_svc::http::Method::Post,
                uri,
                &[("Content-Type", "application/json")],
            )
            .map_err(|e| ClientError::ConnectionError(e))?;

        request
            .write(&body)
            .map_err(|e| ClientError::ConnectionError(e))?;

        let mut response = request
            .submit()
            .map_err(|e| ClientError::ConnectionError(e))?;

        let status_code = response.status();

        let mut buf = [0_u8; 512];

        let size = response
            .read(&mut buf)
            .map_err(|e| ClientError::ConnectionError(e))?;

        let buf = &buf[..size];

        #[cfg(feature = "log")]
        log::info!("{:?}", String::from_utf8(buf.to_vec()));

        match status_code {
            200..=299 => {
                let response = serde_json::from_slice::<D>(buf)?;
                Ok(response)
            }
            _ => {
                let response = serde_json::from_slice::<DexcomErrorResponse>(buf)?;
                let error: DexcomError = response.into();
                Err(ClientError::DexcomError(error))
            }
        }
    }

    pub fn get_current_glucose_reading(
        &mut self,
        session_id: &str,
    ) -> Result<[GlucosReading; 1], C> {
        self.post_request(
            url::DEXCOM_GLUCOSE_READINGS_ENDPOINT,
            &GetLatestGlucoseValuesRequest {
                session_id,
                minutes: 10,
                max_count: 1,
            },
        )
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

#[cfg(test)]
mod tests {
    use core::fmt::Write;

    use super::*;

    // impl Connection for FakeClient {
    //     type Error = core::fmt::Error;
    //     fn post<const N: usize>(
    //         &mut self,
    //         url: &str,
    //         _: &[u8],
    //         buf: &mut Vec<u8, N>,
    //     ) -> Result<u16, Self::Error> {
    //         match url {
    //             url::DEXCOM_GLUCOSE_READINGS_ENDPOINT => {
    //                 buf.write_str(&r#"[{"WT":"Date(1699110415000)","ST":"Date(1699110415000)","DT":"Date(1699110415000+0900)","Value":153,"Trend":"Flat"}]"#)?;
    //                 Ok(200)
    //             }
    //             url::DEXCOM_LOGIN_ID_ENDPOINT => {
    //                 buf.write_str(&r#""a21d18db-a276-40bc-8337-77dcd02df53e""#)?;
    //                 Ok(200)
    //             }
    //             url::DEXCOM_AUTHENTICATE_ENDPOINT => {
    //                 buf.write_str(&r#""1e913fce-5a34-4d27-a991-b6cb3a3bd3d8""#)?;
    //                 Ok(200)
    //             }
    //             _ => unreachable!(),
    //         }
    //     }
    // }

    // #[test]
    // fn test_get_current_glucose_reading() {
    //     let dexcom = Dexcom::new();
    //     let mut client = FakeClient {};

    //     let session_id = dexcom.load_session_id(&mut client, "", "", "").unwrap();
    //     assert_eq!(
    //         session_id,
    //         UUID::from_str("1e913fce-5a34-4d27-a991-b6cb3a3bd3d8").unwrap()
    //     );

    //     let glucose = dexcom.get_current_glucose_reading(&mut client, &session_id);

    //     assert!(glucose.is_ok());
    //     assert_eq!(
    //         glucose,
    //         Ok([GlucosReading {
    //             trend: Trend::Flat,
    //             value: 153,
    //         }])
    //     )
    // }

    // #[test]
    // fn test_dexcom_error_response() {
    //     let message = r#"{"Code":"SessionIdNotFound"}"#;
    //     let (response, _) = serde_json_core::from_str::<DexcomErrorResponse>(message).unwrap();

    //     let error: DexcomError = response.into();
    //     assert_eq!(error, DexcomError::SessionError);
    // }
}
