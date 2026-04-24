use axum::{extract::{Path, State}, http::StatusCode, Json, response::IntoResponse};
use crate::error::ApiError;
use crate::models::carbon::{CarbonConfigUpdate, CarbonConfigResponse, CarbonConfigRow, CarbonRegion};
use crate::state::AppState;
use crate::services::carbon_api::{
    fetch_electricity_maps_intensity, fetch_electricity_maps_price, geocode_address,
    static_intensity, CARBON_REGIONS,
};

/// Helper to convert a raw database row into a public response object.
fn make_carbon_config_response(row: CarbonConfigRow) -> CarbonConfigResponse {
    CarbonConfigResponse {
        has_api_key: row.electricity_maps_api_key.as_deref().map_or(false, |k| !k.is_empty()),
        country_code: row.country_code,
        address: row.address,
        latitude: row.latitude,
        longitude: row.longitude,
        resolved_intensity_g_per_kwh: row.resolved_intensity_g_per_kwh,
        intensity_source: row.intensity_source,
        intensity_fetched_at: row.intensity_fetched_at,
        pue_factor: row.pue_factor,
        manual_cost_per_kwh: row.manual_cost_per_kwh,
        manual_currency: row.manual_currency,
        use_api_for_cost: row.use_api_for_cost,
        updated_at: row.updated_at,
    }
}

/// Handles GET requests to retrieve the carbon footprint config for a specific project.
pub async fn get_carbon_config_handler(
    Path(project_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let row = crate::db::carbon::get_project_carbon_config(&state, project_id).await?;
    Ok(Json(make_carbon_config_response(row)))
}

/// Handles PUT requests to update the carbon configuration for a project.
/// Resolves geolocation and fetches initial real-time intensity if an API key is provided.
pub async fn put_carbon_config_handler(
    Path(project_id): Path<i64>,
    State(state): State<AppState>,
    Json(body): Json<CarbonConfigUpdate>,
) -> Result<impl IntoResponse, ApiError> {
    let (lat, lon, resolved_address) = if let Some(addr) = &body.address {
        match geocode_address(addr).await {
            Ok((lat, lon, display)) => (Some(lat), Some(lon), Some(display)),
            Err(e) => {
                println!("Geocoding failed for {}: {}", addr, e);
                return Err(ApiError::new(
                    StatusCode::BAD_REQUEST,
                    format!("Could not find location for '{}'. Please try a more specific address.", addr),
                ));
            }
        }
    } else {
        (body.latitude, body.longitude, body.address.clone())
    };

    let current = crate::db::carbon::get_project_carbon_config(&state, project_id).await?;
    let api_key = if body.clear_api_key {
        None
    } else {
        body.electricity_maps_api_key.clone()
            .or(current.electricity_maps_api_key.clone())
    };

    let country_code = body.country_code.clone().unwrap_or(current.country_code.clone());

    let (intensity, intensity_src) = if let (Some(lat_val), Some(lon_val), Some(key)) = (lat, lon, api_key.as_deref()) {
        match fetch_electricity_maps_intensity(lat_val, lon_val, key).await {
            Ok(v) => (v, "api".to_string()),
            Err(e) => {
                return Err(ApiError::new(
                    StatusCode::BAD_GATEWAY,
                    format!("Failed to fetch real-time intensity from Electricity Maps: {}. Please check your API key and location.", e),
                ));
            }
        }
    } else {
        (static_intensity(&country_code), format!("static_{country_code}"))
    };

    let pue = body.pue_factor.unwrap_or(current.pue_factor);
    let manual_cost = body.manual_cost_per_kwh.or(current.manual_cost_per_kwh);
    let manual_currency = body.manual_currency.clone().or(current.manual_currency.clone());
    let use_api_for_cost = body.use_api_for_cost.unwrap_or(current.use_api_for_cost);

    let (api_price, api_currency) = if use_api_for_cost {
        if let (Some(lat_val), Some(lon_val), Some(key)) = (lat, lon, api_key.as_deref()) {
            match fetch_electricity_maps_price(lat_val, lon_val, key).await {
                Ok(v) => (Some(v.0), Some(v.1)),
                Err(_) => (None, None),
            }
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    let final_cost = if use_api_for_cost { api_price } else { manual_cost };
    let final_currency = if use_api_for_cost { api_currency } else { manual_currency };

    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE projects SET
            country_code = $1,
            address = $2,
            latitude = $3,
            longitude = $4,
            electricity_maps_api_key = $5,
            resolved_intensity_g_per_kwh = $6,
            intensity_source = $7,
            intensity_fetched_at = $8,
            pue_factor = $9,
            manual_cost_per_kwh = $10,
            manual_currency = $11,
            use_api_for_cost = $12,
            updated_at = $13
         WHERE id = $14",
    )
    .bind(&country_code)
    .bind(&resolved_address)
    .bind(lat)
    .bind(lon)
    .bind(api_key.as_deref())
    .bind(intensity)
    .bind(&intensity_src)
    .bind(if intensity_src == "api" { Some(now.as_str()) } else { None })
    .bind(pue)
    .bind(final_cost)
    .bind(final_currency)
    .bind(use_api_for_cost)
    .bind(&now)
    .bind(project_id)
    .execute(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    let row = crate::db::carbon::get_project_carbon_config(&state, project_id).await?;
    Ok(Json(make_carbon_config_response(row)))
}

/// Handles POST requests to explicitly force a refresh of the real-time carbon intensity from Electricity Maps.
pub async fn post_refresh_carbon_intensity(
    Path(project_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let current = crate::db::carbon::get_project_carbon_config(&state, project_id).await?;
    let (intensity, intensity_src) = match (current.latitude, current.longitude, current.electricity_maps_api_key.as_deref()) {
        (Some(lat), Some(lon), Some(key)) if !key.is_empty() => {
            match fetch_electricity_maps_intensity(lat, lon, key).await {
                Ok(v) => (v, "api".to_string()),
                Err(e) => return Err(ApiError::new(StatusCode::BAD_GATEWAY, format!("Electricity Maps Intensity API error: {e}"))),
            }
        }
        _ => return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "No location and API key configured. Set address and electricity_maps_api_key first.",
        )),
    };

    let (final_cost, final_currency) = if current.use_api_for_cost {
        if let (Some(lat), Some(lon), Some(key)) = (current.latitude, current.longitude, current.electricity_maps_api_key.as_deref()) {
            match fetch_electricity_maps_price(lat, lon, key).await {
                Ok((p, c)) => (Some(p), Some(c)),
                Err(e) => return Err(ApiError::new(StatusCode::BAD_GATEWAY, format!("Electricity Maps Price API error: {e}"))),
            }
        } else {
            (current.manual_cost_per_kwh, current.manual_currency.clone())
        }
    } else {
        (current.manual_cost_per_kwh, current.manual_currency.clone())
    };

    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE projects SET
            resolved_intensity_g_per_kwh = $1,
            intensity_source = $2,
            intensity_fetched_at = $3,
            manual_cost_per_kwh = $4,
            manual_currency = $5,
            updated_at = $6
         WHERE id = $7",
    )
    .bind(intensity)
    .bind(&intensity_src)
    .bind(&now)
    .bind(final_cost)
    .bind(&final_currency)
    .bind(&now)
    .bind(project_id)
    .execute(&state.pool)
    .await
    .map_err(ApiError::internal)?;

    let row = crate::db::carbon::get_project_carbon_config(&state, project_id).await?;
    Ok(Json(make_carbon_config_response(row)))
}

/// Handles GET requests to list all supported static carbon intensity regions.
pub async fn get_carbon_regions_handler() -> impl IntoResponse {
    let regions: Vec<CarbonRegion> = CARBON_REGIONS
        .iter()
        .map(|(code, name, intensity)| CarbonRegion {
            code: code.to_string(),
            name: name.to_string(),
            intensity_g_per_kwh: *intensity,
        })
        .collect();
    Json(regions)
}
