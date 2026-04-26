use crate::models::carbon::{NominatimResult, ElectricityMapsResponse, ElectricityMapsPriceResponse};

/// Static carbon intensity lookup table (gCO₂eq/kWh).
pub const CARBON_REGIONS: &[(&str, &str, f64)] = &[
    ("WORLD", "Global Average", 445.0), // IEA Emissions Factors 2025: https://www.iea.org/reports/electricity-2025/emissions
    ("FR", "France", 52.0),
    ("SE", "Sweden", 13.0),
    ("NO", "Norway", 28.0),
    ("IS", "Iceland", 28.0),
    ("CH", "Switzerland", 43.0),
    ("AT", "Austria", 158.0),
    ("GB", "United Kingdom", 207.0),
    ("IE", "Ireland", 295.0),
    ("PT", "Portugal", 167.0),
    ("ES", "Spain", 163.0),
    ("IT", "Italy", 233.0),
    ("NL", "Netherlands", 295.0),
    ("BE", "Belgium", 167.0),
    ("DK", "Denmark", 156.0),
    ("FI", "Finland", 76.0),
    ("DE", "Germany", 380.0),
    ("PL", "Poland", 635.0),
    ("CZ", "Czech Republic", 425.0),
    ("RO", "Romania", 280.0),
    ("UA", "Ukraine", 296.0),
    ("TR", "Turkey", 400.0),
    ("RU", "Russia", 340.0),
    ("US", "United States", 367.0),
    ("CA", "Canada", 130.0),
    ("MX", "Mexico", 415.0),
    ("BR", "Brazil", 91.0),
    ("AR", "Argentina", 263.0),
    ("CL", "Chile", 290.0),
    ("CO", "Colombia", 175.0),
    ("VE", "Venezuela", 160.0),
    ("ZA", "South Africa", 700.0),
    ("NG", "Nigeria", 430.0),
    ("EG", "Egypt", 450.0),
    ("MA", "Morocco", 540.0),
    ("DZ", "Algeria", 490.0),
    ("AE", "United Arab Emirates", 450.0),
    ("SA", "Saudi Arabia", 530.0),
    ("IL", "Israel", 480.0),
    ("IN", "India", 680.0),
    ("PK", "Pakistan", 430.0),
    ("BD", "Bangladesh", 545.0),
    ("CN", "China", 555.0),
    ("JP", "Japan", 471.0),
    ("KR", "South Korea", 415.0),
    ("TW", "Taiwan", 480.0),
    ("TH", "Thailand", 480.0),
    ("VN", "Vietnam", 360.0),
    ("ID", "Indonesia", 660.0),
    ("MY", "Malaysia", 585.0),
    ("SG", "Singapore", 430.0),
    ("PH", "Philippines", 580.0),
    ("AU", "Australia", 510.0),
    ("NZ", "New Zealand", 147.0),
];

/// Returns the static carbon intensity for a given ISO country code.
/// Defaults to the "WORLD" average if the code is not found.
pub fn static_intensity(country_code: &str) -> f64 {
    let code = country_code.trim().to_uppercase();
    CARBON_REGIONS
        .iter()
        .find(|(c, _, _)| *c == code.as_str())
        .map(|(_, _, v)| *v)
        .unwrap_or(445.0) // IEA 2025 global average fallback
}

/// Geocodes a freeform address string into Latitude and Longitude using the public Nominatim API.
pub async fn geocode_address(address: &str) -> Result<(f64, f64, String), Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .user_agent("EcoCode/1.0 (carbon emission tracker)")
        .build()?;
    let results: Vec<NominatimResult> = client
        .get("https://nominatim.openstreetmap.org/search")
        .query(&[("q", address), ("format", "json"), ("limit", "1")])
        .send()
        .await?
        .json()
        .await?;
    let first = results.into_iter().next().ok_or("No geocoding results found")?;
    let lat: f64 = first.lat.parse()?;
    let lon: f64 = first.lon.parse()?;
    println!("Geocoding successful: {} -> ({}, {})", address, lat, lon);
    Ok((lat, lon, first.display_name))
}

/// Fetches real-time carbon intensity (gCO2eq/kWh) for specific coordinates from the Electricity Maps API.
pub async fn fetch_electricity_maps_intensity(lat: f64, lon: f64, api_key: &str) -> Result<f64, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.electricitymap.org/v3/carbon-intensity/latest?lat={}&lon={}",
        lat, lon
    );
    let res = client
        .get(&url)
        .header("auth-token", api_key)
        .send()
        .await?;
    if !res.status().is_success() {
        let msg = res.text().await?;
        println!("Electricity Maps Intensity API error: {}", msg);
        return Err(format!("Intensity API error: {}", msg).into());
    }
    let data: ElectricityMapsResponse = res.json().await?;
    println!("Fetched real-time intensity: {} gCO2/kWh", data.carbon_intensity);
    Ok(data.carbon_intensity)
}

/// Fetches real-time electricity price for specific coordinates from the Electricity Maps API.
pub async fn fetch_electricity_maps_price(lat: f64, lon: f64, api_key: &str) -> Result<(f64, String), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.electricitymap.org/v3/power-price/latest?lat={}&lon={}",
        lat, lon
    );
    let res = client
        .get(&url)
        .header("auth-token", api_key)
        .send()
        .await?;
    if !res.status().is_success() {
        let msg = res.text().await?;
        println!("Electricity Maps Price API error: {}", msg);
        return Err(format!("Price API error: {}", msg).into());
    }
    let data: ElectricityMapsPriceResponse = res.json().await?;
    println!("Fetched real-time price: {} {}", data.price, data.currency);
    Ok((data.price, data.currency))
}
