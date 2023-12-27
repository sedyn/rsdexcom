use embedded_svc::{http::Method, io::Error};
#[cfg(test)]
use mockall::automock;

#[cfg(test)]
#[derive(Debug)]
pub struct TestError;

#[cfg(test)]
impl Error for TestError {
    fn kind(&self) -> embedded_svc::io::ErrorKind {
        embedded_svc::io::ErrorKind::Other
    }
}

#[cfg_attr(test, automock(type Error=TestError;))]
pub trait Client {
    type Error: Error;

    fn request<'a>(
        &mut self,
        method: Method,
        uri: &'a str,
        headers: &'a [(&'a str, &'a str)],
        body: &[u8],
        buf: &mut [u8],
    ) -> Result<(usize, u16), Self::Error>;
}
