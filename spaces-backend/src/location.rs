use std::{sync::Arc, time::Duration};

use anyhow::bail;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

#[derive(Serialize, Deserialize, Debug)]
pub struct Location {
    country: String,
    region: String,
    city: String,
    using_vpn: bool,
}

const IP_API_RATE_LIM_PER_MINUTE: usize = 35; // docs say 45 but let's be safe

pub struct LocationClient {
    client: Client,
    rate_lim_sem: Arc<Semaphore>,
}

impl LocationClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("spaces/1.0 (austinweeks.dev)")
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
            rate_lim_sem: Arc::new(Semaphore::new(IP_API_RATE_LIM_PER_MINUTE)),
        }
    }

    pub async fn query_location(&self, ip_addr: &str) -> anyhow::Result<Location> {
        let permit = self.rate_lim_sem.clone().acquire_owned().await?;
        let url = Url::parse_with_params(
            &format!("http://ip-api.com/json/{}", ip_addr),
            &[("fields", "status,country,regionName,city,proxy")],
        )?;

        let req = self.client.get(url);
        // FIXME: this works (I think) but very ugly and likely inefficient
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_mins(1)).await;
            drop(permit);
        });

        let loc_resp = req
            .send()
            .await
            .and_then(|r| r.error_for_status())?
            .json::<LocResp>()
            .await?;
        let LocResp::Success {
            country,
            region_name,
            city,
            proxy,
        } = loc_resp
        else {
            bail!("ip-api responded but failed to find location");
        };

        Ok(Location {
            country,
            city,
            region: region_name,
            using_vpn: proxy,
        })
    }
}

#[derive(Deserialize)]
#[serde(tag = "status")]
enum LocResp {
    #[serde(rename = "success")]
    Success {
        country: String,
        #[serde(rename = "regionName")]
        region_name: String,
        city: String,
        proxy: bool,
    },
    #[serde(rename = "fail")]
    Fail,
}
