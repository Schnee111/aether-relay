use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventStatus {
    Received,
    Processing,
    Delivered,
    Failed,
}

impl EventStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Received => "RECEIVED",
            Self::Processing => "PROCESSING",
            Self::Delivered => "DELIVERED",
            Self::Failed => "FAILED",
        }
    }
}

impl FromStr for EventStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "RECEIVED" => Ok(Self::Received),
            "PROCESSING" => Ok(Self::Processing),
            "DELIVERED" => Ok(Self::Delivered),
            "FAILED" => Ok(Self::Failed),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointRecord {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub secret: String,
    pub target_url: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingEventRecord {
    pub id: String,
    pub endpoint_id: String,
    pub idempotency_key: String,
    pub raw_body: Vec<u8>,
    pub headers: String,
    pub status: EventStatus,
    pub created_at: i64,
}
