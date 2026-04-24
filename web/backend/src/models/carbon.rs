use serde::{Deserialize, Serialize};

/// Holds the raw row representing carbon footprint configuration for a project.
#[derive(Serialize, Clone, sqlx::FromRow, Debug)]
pub struct CarbonConfigRow {
    pub country_code: String,
    pub address: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub electricity_maps_api_key: Option<String>,
    pub resolved_intensity_g_per_kwh: f64,
    pub intensity_source: String,
    pub intensity_fetched_at: Option<String>,
    pub pue_factor: f64,
    pub manual_cost_per_kwh: Option<f64>,
    pub manual_currency: Option<String>,
    pub use_api_for_cost: bool,
    pub updated_at: String,
}

/// Public response object representing carbon footprint configuration.
#[derive(Serialize, Clone, Debug)]
pub struct CarbonConfigResponse {
    pub country_code: String,
    pub address: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub has_api_key: bool,
    pub resolved_intensity_g_per_kwh: f64,
    pub intensity_source: String,
    pub intensity_fetched_at: Option<String>,
    pub pue_factor: f64,
    pub manual_cost_per_kwh: Option<f64>,
    pub manual_currency: Option<String>,
    pub use_api_for_cost: bool,
    pub updated_at: String,
}

/// Request payload for updating the carbon footprint configuration.
#[derive(Deserialize, Debug)]
pub struct CarbonConfigUpdate {
    pub address: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub country_code: Option<String>,
    pub electricity_maps_api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
    pub pue_factor: Option<f64>,
    pub manual_cost_per_kwh: Option<f64>,
    pub manual_currency: Option<String>,
    pub use_api_for_cost: Option<bool>,
}

/// Represents a static carbon intensity region.
#[derive(Serialize, Clone, Debug)]
pub struct CarbonRegion {
    pub code: String,
    pub name: String,
    pub intensity_g_per_kwh: f64,
}

/// Geocoding result from Nominatim API.
#[derive(Deserialize, Debug)]
pub struct NominatimResult {
    pub lat: String,
    pub lon: String,
    pub display_name: String,
}

/// Electricity Maps intensity API response.
#[derive(Deserialize, Debug)]
pub struct ElectricityMapsResponse {
    #[serde(rename = "carbonIntensity")]
    pub carbon_intensity: f64,
}

/// Electricity Maps price API response.
#[derive(Deserialize, Debug)]
pub struct ElectricityMapsPriceResponse {
    pub price: f64,
    pub currency: String,
}
