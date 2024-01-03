use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use serde::Deserialize;

use crate::primitives::Centimeter;

/// Configuration data for the whole motorized standing desk.
#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub table: TableConfig,
    pub sensor: SensorConfig,
    pub motor: MotorConfig,
}

/// Configuration data for the standing desk.
#[derive(Debug, Deserialize)]
pub(crate) struct TableConfig {
    pub max_table_height: Centimeter,
    pub min_table_height: Centimeter,
    pub sitting_height: Centimeter,
    pub standing_height: Centimeter,
}

/// Configuration data for the distance sensor.
#[derive(Debug, Deserialize)]
pub(crate) struct SensorConfig {
    // The pin number controlling the distance sensor's trigger
    pub trigger_pin: u8,
    // The pin number listening for the distance sensor's echo signal
    pub echo_pin: u8,
    // The calibration file for the sensor
    pub calibration_file: PathBuf,
}

/// Configuration data for the standing desk motor.
#[derive(Debug, Deserialize)]
pub(crate) struct MotorConfig {
    // The pin number controlling the motor's upwards movement
    pub up_pin: u8,
    // The pin number controlling the motor's downwards movement
    pub down_pin: u8,
}

impl Config {
    /// Loads a configuration from a file.
    pub(crate) fn load<P: AsRef<Path>>(path: P) -> Result<Config> {
        let raw_config = fs::read_to_string(path)?;
        let config = toml::from_str(&raw_config)?;
        Ok(config)
    }
}
