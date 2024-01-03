use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;
use std::time::SystemTime;

use anyhow::anyhow;
use anyhow::Result;
use log::debug;
use rppal::gpio::Gpio;
use rppal::gpio::InputPin;
use rppal::gpio::OutputPin;
use rppal::gpio::Trigger;
use serde::Deserialize;
use serde::Serialize;

use crate::config::SensorConfig;
use crate::primitives::Centimeter;

pub(crate) trait DistanceSensor {
    /// Takes a height measurement in centimeters.
    fn current_height(&mut self) -> Result<Centimeter>;

    /// Sets the lowest height as reference for calibration.
    fn set_min_height(
        &mut self,
        height: Centimeter,
    ) -> Result<()>;

    /// Sets the highest height as reference for calibration.
    fn set_max_height(
        &mut self,
        height: Centimeter,
    ) -> Result<()>;

    fn calibration_file(&self) -> &Path;

    fn calibration_data(&self) -> &SensorCalibrationData;
}

/// The HCSR04 sensor for measuring distances.
pub(crate) struct HCSR04 {
    calibration_file_path: PathBuf,
    calibration_data: SensorCalibrationData,
    trigger_pin: OutputPin,
    echo_pin: InputPin,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct SensorCalibrationData {
    // The minimum height we can observe
    pub min_height: Centimeter,
    // The duration of the echo in ms at minimum height
    pub min_height_echo: Duration,
    // The max height we can observe
    pub max_height: Centimeter,
    // The duration of the echo in ms at max height
    pub max_height_echo: Duration,
}

impl SensorCalibrationData {
    /// Loads calibration data from a file.
    pub(crate) fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let raw_data = fs::read_to_string(path)?;
        let calibration = toml::from_str(&raw_data)?;
        Ok(calibration)
    }
}

impl HCSR04 {
    /// Creates a new [HCSR04] with calibration parameters from the file.
    pub(crate) fn new(config: SensorConfig) -> Self {
        let gpio = Gpio::new().expect("gpio to be available");
        let calibration_file_path = config.calibration_file;
        let calibration_data = SensorCalibrationData::load(&calibration_file_path)
            .expect("calibration data must be available");
        Self {
            calibration_file_path,
            calibration_data,
            trigger_pin: gpio
                .get(config.trigger_pin)
                .expect("trigger pin be available")
                .into_output(),
            echo_pin: gpio
                .get(config.echo_pin)
                .expect("echo pin be available")
                // Echo should be on low per default
                .into_input_pulldown(),
        }
    }

    /// Measures the time it takes for the sensor to send and receive an acoustic echo.
    /// # Errors
    /// Errors if there is no object close enough or the object is too small.
    fn measure_full_echo_duration(&mut self) -> Result<Duration> {
        // We want to block on both rising and falling signal edges which indicate the start and end
        // of a measurement respectively.
        // TODO move this into the constructor?
        self.echo_pin.set_interrupt(Trigger::Both)?;

        // TODO add mechanism to prevent too frequent calls of this function
        self.trigger_pin.set_high();
        // Trigger needs to be set to high for at least 10us, let's be certain here with 100us.
        sleep(Duration::from_micros(100));
        // A falling signal edge is the actual trigger for the sensor to start the measurement.
        self.trigger_pin.set_low();
        // Wait for the rising edge indicating the start of the measurement.
        // We expect a delay of around 500us as per the datasheet:
        // https://www.mikrocontroller.net/attachment/218122/HC-SR04_ultraschallmodul_beschreibung_3.pdf
        self.echo_pin
            .poll_interrupt(true, Some(Duration::from_millis(1)))?;
        let start_time = SystemTime::now();
        // Let's wait for the falling edge indicating the end of the measurement.
        // No need to reset the interrupt as we've just received the last event.
        // Timeout is 250ms as the sensor should return to low after 200ms max to indicate an
        // unsuccessful measurement.
        self.echo_pin
            .poll_interrupt(false, Some(Duration::from_millis(250)))?;
        let echo_duration = start_time.elapsed()?;
        if echo_duration >= Duration::from_millis(200) {
            return Err(anyhow!("unsuccessful measurement"));
        }
        Ok(echo_duration)
    }
}

impl DistanceSensor for HCSR04 {
    fn current_height(&mut self) -> Result<Centimeter> {
        let echo_duration = self.measure_full_echo_duration()?;
        // We're interpolating the height from our calibration parameters
        let normalized_echo = (echo_duration - self.calibration_data.min_height_echo).as_micros()
            as f32
            / (self.calibration_data.max_height_echo - self.calibration_data.min_height_echo)
                .as_micros() as f32;
        let height = normalized_echo
            * (self.calibration_data.max_height - self.calibration_data.min_height).into_inner()
                as f32
            + self.calibration_data.min_height.into_inner() as f32;
        let height = Centimeter(height.round() as u8);
        debug!("Current height is {height:?}");
        Ok(height)
    }

    fn set_min_height(
        &mut self,
        height: Centimeter,
    ) -> Result<()> {
        debug!("Setting min height {height:?}");
        let echo_duration = self.measure_full_echo_duration()?;
        debug!("Min height echo duration: {echo_duration:?}");
        self.calibration_data.min_height_echo = echo_duration;
        self.calibration_data.min_height = height;
        Ok(())
    }

    fn set_max_height(
        &mut self,
        height: Centimeter,
    ) -> Result<()> {
        debug!("Setting max height {height:?}");
        let echo_duration = self.measure_full_echo_duration()?;
        debug!("Max height echo duration: {echo_duration:?}");
        self.calibration_data.max_height_echo = echo_duration;
        self.calibration_data.max_height = height;
        Ok(())
    }

    fn calibration_file(&self) -> &Path {
        &self.calibration_file_path
    }

    fn calibration_data(&self) -> &SensorCalibrationData {
        &self.calibration_data
    }
}
