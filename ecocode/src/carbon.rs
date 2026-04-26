// Carbon emission calculation for EcoCode.
// Formula: CO₂ (g) = energy_wh / 1000 × intensity_g_per_kwh × pue

/// Static carbon intensity lookup by country code (ISO 3166-1 alpha-2).
/// Values in gCO₂eq/kWh, sourced from IEA / carbonfootprint.com 2024-2025 data.
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

/// Returns the carbon intensity in gCO₂eq/kWh for the given country code.
/// Falls back to the global average ("WORLD") if the code is not found.
pub fn static_intensity(country_code: &str) -> f64 {
    let code = country_code.trim().to_uppercase();
    CARBON_REGIONS
        .iter()
        .find(|(c, _, _)| *c == code.as_str())
        .map(|(_, _, intensity)| *intensity)
        .unwrap_or(445.0) // WORLD fallback (IEA 2025)
}

/// Computes CO₂ emissions in grams.
///
/// - `energy_wh`: total energy in Wh (sum of cpu + gpu + mem + igpu energy, already converted from J)
/// - `intensity_g_per_kwh`: carbon intensity of the grid in gCO₂eq/kWh
/// - `pue`: Power Usage Effectiveness factor (1.0 for workstations, ~1.58 for avg data center)
pub fn calculate_co2_g(energy_wh: f64, intensity_g_per_kwh: f64, pue: f64) -> f64 {
    energy_wh / 1000.0 * intensity_g_per_kwh * pue
}
