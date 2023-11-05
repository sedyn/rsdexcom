#![no_std]

pub mod url;

use core::{marker::PhantomData, str::FromStr};

use heapless::{String, Vec};
use serde::{Deserialize, Serialize};

pub type UUID = String<36>;

pub trait HttpClient {
    type Error;

    fn post<const N: usize>(
        &mut self,
        url: &str,
        body: &[u8],
        buf: &mut Vec<u8, N>,
    ) -> Result<u16, Self::Error>;
}

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
    ParsingError,
    NetworkError,
    UnknownError,
}

type DexcomResult<T> = Result<T, DexcomError>;

pub struct Dexcom<T: HttpClient> {
    _client: PhantomData<T>,
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

impl<T: HttpClient> Dexcom<T> {
    pub fn new() -> Self {
        Self {
            _client: PhantomData,
        }
    }

    fn post_request<R: Serialize, const N: usize>(
        &self,
        client: &mut T,
        url: &str,
        request: &R,
        buf: &mut Vec<u8, N>,
    ) -> DexcomResult<()> {
        let body =
            serde_json_core::to_vec::<_, 128>(&request).map_err(|_| DexcomError::ParsingError)?;

        let status_code = client
            .post::<N>(url, &body, buf)
            .map_err(|_| DexcomError::NetworkError)?;

        match status_code {
            200..=299 => Ok(()),
            _ => {
                let (response, _) = serde_json_core::from_slice::<DexcomErrorResponse>(buf)
                    .map_err(|_| DexcomError::ParsingError)?;

                Err(response.into())
            }
        }
    }

    pub fn get_current_glucose_reading(
        &self,
        client: &mut T,
        session_id: &str,
    ) -> DexcomResult<[GlucosReading; 1]> {
        let request = GetLatestGlucoseValuesRequest {
            session_id,
            minutes: 10,
            max_count: 1,
        };

        let mut buf: Vec<u8, 256> = Vec::new();

        self.post_request(
            client,
            url::DEXCOM_GLUCOSE_READINGS_ENDPOINT,
            &request,
            &mut buf,
        )?;

        let (response, _) =
            serde_json_core::from_slice(&buf).map_err(|_| DexcomError::ParsingError)?;
        Ok(response)
    }

    pub fn load_session_id(
        &self,
        client: &mut T,
        account_name: &str,
        password: &str,
        application_id: &str,
    ) -> DexcomResult<UUID> {
        let account_id = self.get_account_id(client, account_name, password, application_id)?;
        let session_id = self.get_session_id(client, &account_id, password, application_id)?;
        Ok(session_id)
    }

    fn get_account_id(
        &self,
        client: &mut T,
        account_name: &str,
        password: &str,
        application_id: &str,
    ) -> DexcomResult<UUID> {
        let request = GetAccountIdRequest {
            account_name,
            password,
            application_id,
        };

        let mut buf: Vec<u8, 256> = Vec::new();

        self.post_request(client, url::DEXCOM_LOGIN_ID_ENDPOINT, &request, &mut buf)?;

        let (response, _) =
            serde_json_core::from_slice(&buf).map_err(|_| DexcomError::ParsingError)?;

        let response = UUID::from_str(response).map_err(|_| DexcomError::ParsingError)?;

        Ok(response)
    }

    fn get_session_id(
        &self,
        client: &mut T,
        account_id: &str,
        password: &str,
        application_id: &str,
    ) -> DexcomResult<UUID> {
        let request = GetSessionIdRequest {
            account_id,
            password,
            application_id,
        };

        let mut buf: Vec<u8, 256> = Vec::new();

        self.post_request(
            client,
            url::DEXCOM_AUTHENTICATE_ENDPOINT,
            &request,
            &mut buf,
        )?;

        let (response, _) =
            serde_json_core::from_slice(&buf).map_err(|_| DexcomError::ParsingError)?;

        let response = UUID::from_str(response).map_err(|_| DexcomError::ParsingError)?;

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use core::fmt::Write;

    use super::*;

    struct FakeClient {}

    impl HttpClient for FakeClient {
        type Error = core::fmt::Error;
        fn post<const N: usize>(
            &mut self,
            url: &str,
            _: &[u8],
            buf: &mut Vec<u8, N>,
        ) -> Result<u16, Self::Error> {
            match url {
                url::DEXCOM_GLUCOSE_READINGS_ENDPOINT => {
                    buf.write_str(&r#"[{"WT":"Date(1699110415000)","ST":"Date(1699110415000)","DT":"Date(1699110415000+0900)","Value":153,"Trend":"Flat"}]"#)?;
                    Ok(200)
                }
                url::DEXCOM_LOGIN_ID_ENDPOINT => {
                    buf.write_str(&r#""a21d18db-a276-40bc-8337-77dcd02df53e""#)?;
                    Ok(200)
                }
                url::DEXCOM_AUTHENTICATE_ENDPOINT => {
                    buf.write_str(&r#""1e913fce-5a34-4d27-a991-b6cb3a3bd3d8""#)?;
                    Ok(200)
                }
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn test_get_current_glucose_reading() {
        let dexcom = Dexcom::new();
        let mut client = FakeClient {};

        let session_id = dexcom.load_session_id(&mut client, "", "", "").unwrap();
        assert_eq!(
            session_id,
            UUID::from_str("1e913fce-5a34-4d27-a991-b6cb3a3bd3d8").unwrap()
        );

        let glucose = dexcom.get_current_glucose_reading(&mut client, &session_id);

        assert!(glucose.is_ok());
        assert_eq!(
            glucose,
            Ok([GlucosReading {
                trend: Trend::Flat,
                value: 153,
            }])
        )
    }

    #[test]
    fn test_dexcom_error_response() {
        let message = r#"{"Code":"SessionIdNotFound"}"#;
        let (response, _) = serde_json_core::from_str::<DexcomErrorResponse>(message).unwrap();

        let error: DexcomError = response.into();
        assert_eq!(error, DexcomError::SessionError);
    }
}
