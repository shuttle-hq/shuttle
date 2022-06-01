use std::fmt::Display;
use std::net::IpAddr;
use hyper::body::{Body, Bytes};
use hyper::{StatusCode, Uri};
use serde::Deserialize;
use shuttle_common::DeploymentMeta;
use futures::prelude::*;
use hyper::client::HttpConnector;

use crate::{API_PORT, Error, ErrorKind};

pub struct Client {
    target: IpAddr,
    hyper: hyper::Client<HttpConnector, Body>
}

impl Client {
    pub fn new(from: &hyper::Client<HttpConnector, Body>, target: IpAddr) -> Self {
        Self {
            target,
            hyper: from.clone()
        }
    }

    async fn get<T: for<'de> Deserialize<'de>>(&self, uri: Uri) -> Result<T, Error> {
        let resp = self.hyper.get(uri).await.unwrap();
        if resp.status() != StatusCode::OK {
            Err(Error::from(ErrorKind::Internal))
        } else {
            let body = resp
                .into_body()
                .try_fold(Vec::new(), |mut buf, bytes|  async move {
                    buf.extend(bytes.into_iter());
                    Ok(buf)
                })
                .await
                .unwrap();
            let t = serde_json::from_slice(&body).unwrap();
            Ok(t)
        }
    }

    pub async fn active_port<S: Display>(&self, service: S) -> Option<u16> {
        let uri = format!("http://{}:{}/services/{}", self.target, API_PORT, service).parse().unwrap();
        self.get::<DeploymentMeta>(uri).await.unwrap().port
    }
}