pub mod url;

use anyhow::{bail, Result};

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

#[repr(u8)]
#[derive(Debug, PartialEq)]
pub enum DexcomError {
    AccountError,
    SessionError,
    ArgumentError,
    UnknownError,
}

pub struct Dexcom<'a, C: Connection>
where
    <C as ErrorType>::Error: std::error::Error + Send + Sync + 'static,
{
    client: &'a mut Client<C>,
}

#[derive(Serialize)]
struct GetLatestGlucoseValuesRequest<'a> {
    session_id: &'a str,
    // fixed as 10
    minutes: u32,
    // fixed as 1.
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

impl<'a, C: Connection> Dexcom<'a, C>
where
    <C as ErrorType>::Error: std::error::Error + Send + Sync + 'static,
{
    pub fn new(client: &'a mut Client<C>) -> Self {
        Self { client }
    }

    fn post_request<S: Serialize, D: DeserializeOwned>(
        &mut self,
        uri: &str,
        request: &S,
    ) -> Result<D> {
        let body = serde_json::to_vec(&request)?;

        let mut request = self
            .client
            .request(embedded_svc::http::Method::Post, uri, &[])?;

        request.write(&body)?;

        let mut response = request.submit()?;

        let status_code = response.status();

        let mut buf = [0_u8; 256];

        response.read(&mut buf)?;

        match status_code {
            200..=299 => {
                let response = serde_json::from_slice::<D>(&buf)?;
                Ok(response)
            }
            _ => {
                let response = serde_json::from_slice::<DexcomErrorResponse>(&buf)?;
                let error: DexcomError = response.into();
                bail!("{:?}", error)
            }
        }
    }

    pub fn get_current_glucose_reading(&mut self, session_id: &str) -> Result<[GlucosReading; 1]> {
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
    ) -> Result<String> {
        let account_id = self.get_account_id(account_name, password, application_id)?;
        let session_id = self.get_session_id(&account_id, password, application_id)?;
        Ok(session_id)
    }

    fn get_account_id(
        &mut self,
        account_name: &str,
        password: &str,
        application_id: &str,
    ) -> Result<String> {
        self.post_request(
            url::DEXCOM_LOGIN_ID_ENDPOINT,
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
    ) -> Result<String> {
        self.post_request(
            url::DEXCOM_AUTHENTICATE_ENDPOINT,
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
